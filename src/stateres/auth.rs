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

use crate::{
	event::Event,
	power::PowerLevels,
	state_map::{EventId, EventStore, StateMap},
};

/// The state key of the room's power-levels event.
const POWER_LEVELS_KEY: (&str, &str) = ("m.room.power_levels", "");

/// The room's create event type.
const CREATE_TYPE: &str = "m.room.create";

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
