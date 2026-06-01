//! Iterative authorisation checks.
//!
//! This is the step that consumes the resolution orderings
//! ([`reverse_topological_power_sort`](crate::reverse_topological_power_sort)
//! and [`mainline_ordering`](crate::mainline_ordering)) to build the resolved
//! state: events are walked in order, and each is folded into the state only if
//! the room-version authorisation rules accept it against the state resolved so
//! far (specification §III-D).
//!
//! The rules themselves are large and room-version-specific, so the engine takes
//! them as an injected [`AuthRules`] implementation — keeping the deterministic
//! resolution flow independent of, and testable without, the full rule set. A
//! concrete `AuthRules` for room versions 1–12 is the next increment and plugs
//! in here unchanged.

use std::collections::BTreeSet;

use crate::{
	event::Event,
	power::PowerLevels,
	state_map::{EventId, EventStore, StateMap},
};

/// The state key of the room's power-levels event.
const POWER_LEVELS_KEY: (&str, &str) = ("m.room.power_levels", "");

/// The room's create event type.
const CREATE_TYPE: &str = "m.room.create";

/// The member event type.
const MEMBER_TYPE: &str = "m.room.member";

/// The join-rules state key.
const JOIN_RULES_KEY: (&str, &str) = ("m.room.join_rules", "");

/// The room-version authorisation rules: decide whether `event` is authorised
/// against the partially-resolved `state`.
///
/// `store` is provided so a rule can read the content of state events (the
/// resolved `state` maps slots to event ids, not content).
pub trait AuthRules<E: Event> {
	/// Whether `event` is authorised given the state resolved so far.
	fn is_authorized(&self, event: &E, state: &StateMap, store: &EventStore<E>) -> bool;
}

/// Power-level authorisation: a sender may send an event only if their power
/// level meets the level the room's `m.room.power_levels` state requires for
/// that event type.
///
/// This is one component of the full room-version rule set. When no
/// power-levels event is in the state yet (room bootstrap), it falls back to a
/// default (all-zero) power-levels, which authorises; the create-event and
/// membership rules that govern bootstrap and joins are separate components.
pub struct PowerLevelRules;

impl<E: Event> AuthRules<E> for PowerLevelRules {
	fn is_authorized(&self, event: &E, state: &StateMap, store: &EventStore<E>) -> bool {
		let levels = current_power_levels(state, store);
		let required = levels.required_for(event.event_type(), true);

		levels.for_user(event.sender()) >= required
	}
}

/// Create-event authorisation: the `m.room.create` event is the room root (no
/// auth events, empty state key), and every other event requires the room to
/// have been created (an `m.room.create` event present in the state).
pub struct CreateRules;

impl<E: Event> AuthRules<E> for CreateRules {
	fn is_authorized(&self, event: &E, state: &StateMap, _store: &EventStore<E>) -> bool {
		if event.event_type() == CREATE_TYPE {
			event.auth_event_ids().is_empty() && event.state_key().is_empty()
		} else {
			state.contains_key(&(CREATE_TYPE.to_owned(), String::new()))
		}
	}
}

/// Authorise an event only if **every** contained rule authorises it.
///
/// The room-version rules are a set of gates an event must pass; `AllOf` lets
/// the individual rule components (create, power levels, and — in a later
/// increment — membership) be composed and applied together.
pub struct AllOf<'rules, E: Event>(pub &'rules [&'rules dyn AuthRules<E>]);

impl<E: Event> AuthRules<E> for AllOf<'_, E> {
	fn is_authorized(&self, event: &E, state: &StateMap, store: &EventStore<E>) -> bool {
		self.0
			.iter()
			.all(|rule| rule.is_authorized(event, state, store))
	}
}

/// Membership authorisation for `m.room.member` events: governs the
/// join/invite/leave/ban transitions against the join rule, current
/// memberships, and the inviting/kicking/banning power levels. Non-member
/// events are passed through (this rule only governs membership).
///
/// Scope: this covers the steady-state transitions. The create-room bootstrap
/// join (the room creator's first join), knock, restricted joins, and
/// third-party invites are separate components and not yet handled.
pub struct MembershipRules;

impl<E: Event> AuthRules<E> for MembershipRules {
	fn is_authorized(&self, event: &E, state: &StateMap, store: &EventStore<E>) -> bool {
		if event.event_type() != MEMBER_TYPE {
			return true;
		}

		let Some(membership) = event.membership() else {
			return false;
		};

		let sender = event.sender();
		let target = event.state_key();
		let levels = current_power_levels(state, store);
		let sender_level = levels.for_user(sender);
		let sender_membership = membership_of(sender, state, store);
		let target_membership = membership_of(target, state, store);

		match membership {
			| "join" => {
				if sender != target || target_membership == "ban" {
					return false;
				}
				// Create-room bootstrap: the creator's initial join, whose only
				// auth dependency is the create event.
				if is_bootstrap_join(event, state, store) {
					return true;
				}
				// An existing invite or membership satisfies any non-public rule.
				if matches!(target_membership.as_str(), "invite" | "join") {
					return true;
				}
				match current_join_rule(state, store).as_str() {
					| "public" => true,
					// Restricted joins: a joined user with invite power vouches
					// for the join via join_authorised_via_users_server. (The
					// allowed-rooms membership check is a server-side join-time
					// concern, not part of event authorisation.)
					| "restricted" | "knock_restricted" =>
						event.join_authorised_via_users_server().is_some_and(|authoriser| {
							membership_of(authoriser, state, store) == "join"
								&& levels.for_user(authoriser) >= levels.invite
						}),
					| _ => false,
				}
			},
			| "invite" =>
				sender_membership == "join"
					&& !matches!(target_membership.as_str(), "join" | "ban")
					&& sender_level >= levels.invite,
			| "leave" =>
				if sender == target {
					matches!(sender_membership.as_str(), "join" | "invite" | "knock")
				} else {
					sender_membership == "join"
						&& sender_level >= levels.kick
						&& sender_level > levels.for_user(target)
				},
			| "ban" =>
				sender_membership == "join"
					&& sender_level >= levels.ban
					&& sender_level > levels.for_user(target),
			| "knock" =>
				sender == target
					&& matches!(
						current_join_rule(state, store).as_str(),
						"knock" | "knock_restricted"
					) && !matches!(target_membership.as_str(), "join" | "ban"),
			| _ => false,
		}
	}
}

/// Whether `event` is the room creator's initial join: the joining user is the
/// create event's sender, and this join's only auth dependency is that create
/// event. This is the bootstrap that lets a freshly-created room be joined
/// before any power-levels or join-rules state exists.
fn is_bootstrap_join<E: Event>(event: &E, state: &StateMap, store: &EventStore<E>) -> bool {
	let create_key = (CREATE_TYPE.to_owned(), String::new());
	let Some(create_id) = state.get(&create_key) else {
		return false;
	};

	let creator = store.get(create_id).map(Event::sender);
	creator == Some(event.sender()) && event.auth_event_ids() == std::slice::from_ref(create_id)
}

/// The membership of `user` in `state` (`leave` when no member event is
/// present).
fn membership_of<E: Event>(user: &str, state: &StateMap, store: &EventStore<E>) -> String {
	state
		.get(&(MEMBER_TYPE.to_owned(), user.to_owned()))
		.and_then(|id| store.get(id))
		.and_then(Event::membership)
		.unwrap_or("leave")
		.to_owned()
}

/// The room's join rule in `state` (`invite` when no join-rules event is
/// present, matching the Matrix default).
fn current_join_rule<E: Event>(state: &StateMap, store: &EventStore<E>) -> String {
	let key = (JOIN_RULES_KEY.0.to_owned(), JOIN_RULES_KEY.1.to_owned());
	state
		.get(&key)
		.and_then(|id| store.get(id))
		.and_then(Event::join_rule)
		.unwrap_or("invite")
		.to_owned()
}

/// Power-level mutation constraints for `m.room.power_levels` events: a sender
/// may not set any level higher than their own current level, nor change a
/// level that is already higher than their own. This blocks privilege
/// escalation via a power-levels change. Non-power-levels events pass through.
///
/// Scope: enforces the core "every changed level must be within the sender's
/// reach" rule. The secondary constraint that a user may not act on another
/// user whose level equals their own is not yet enforced.
pub struct PowerLevelMutationRules;

impl<E: Event> AuthRules<E> for PowerLevelMutationRules {
	fn is_authorized(&self, event: &E, state: &StateMap, store: &EventStore<E>) -> bool {
		if event.event_type() != POWER_LEVELS_KEY.0 {
			return true;
		}

		let Some(new_levels) = event.power_levels() else {
			return false;
		};

		let old_levels = current_power_levels(state, store);
		let sender_level = old_levels.for_user(event.sender());

		level_changes(&old_levels, new_levels)
			.into_iter()
			.all(|(old, new)| old <= sender_level && new <= sender_level)
	}
}

/// The `(old, new)` value of every power level that differs between two
/// power-levels contents — the scalar levels and each per-user and per-event
/// entry (resolving omitted entries to their respective defaults).
fn level_changes(old: &PowerLevels, new: &PowerLevels) -> Vec<(i64, i64)> {
	let mut changes = Vec::new();

	let scalars = [
		(old.users_default, new.users_default),
		(old.events_default, new.events_default),
		(old.state_default, new.state_default),
		(old.invite, new.invite),
		(old.kick, new.kick),
		(old.ban, new.ban),
	];
	changes.extend(scalars.into_iter().filter(|(o, n)| o != n));

	let user_keys: BTreeSet<&String> = old.users.keys().chain(new.users.keys()).collect();
	for key in user_keys {
		let o = old.users.get(key).copied().unwrap_or(old.users_default);
		let n = new.users.get(key).copied().unwrap_or(new.users_default);
		if o != n {
			changes.push((o, n));
		}
	}

	let event_keys: BTreeSet<&String> = old.events.keys().chain(new.events.keys()).collect();
	for key in event_keys {
		let o = old.events.get(key).copied().unwrap_or(old.events_default);
		let n = new.events.get(key).copied().unwrap_or(new.events_default);
		if o != n {
			changes.push((o, n));
		}
	}

	changes
}

/// The power levels in effect in `state`, or the default when no power-levels
/// event is present.
fn current_power_levels<E: Event>(state: &StateMap, store: &EventStore<E>) -> PowerLevels {
	let key = (POWER_LEVELS_KEY.0.to_owned(), POWER_LEVELS_KEY.1.to_owned());
	state
		.get(&key)
		.and_then(|id| store.get(id))
		.and_then(|event| event.power_levels().cloned())
		.unwrap_or_default()
}

/// Walk `events` in resolution order, folding each authorised event into `base`
/// state and skipping the rest.
///
/// An event is accepted when `rules` authorise it against the state resolved so
/// far; on acceptance it becomes the state at its `(event_type, state_key)`
/// slot, so later events are checked against it. Events absent from `store` are
/// skipped. The input order is the contract: it must be a
/// [`reverse_topological_power_sort`](crate::reverse_topological_power_sort) or
/// [`mainline_ordering`](crate::mainline_ordering) of the conflicted events.
#[must_use]
pub fn iterative_auth_checks<E, R>(
	events: &[EventId],
	base: StateMap,
	store: &EventStore<E>,
	rules: &R,
) -> StateMap
where
	E: Event,
	R: AuthRules<E>,
{
	let mut state = base;
	for id in events {
		let Some(event) = store.get(id) else {
			continue;
		};

		if rules.is_authorized(event, &state, store) {
			let slot = (event.event_type().to_owned(), event.state_key().to_owned());
			state.insert(slot, id.clone());
		}
	}

	state
}
