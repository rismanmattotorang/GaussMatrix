//! # gm-stateres — GaussMatrix state-resolution engine (foundation)
//!
//! State resolution dominates a homeserver's cost: on receiving a remote event
//! the server must locate its authorisation chain, verify signatures, and re-run
//! conflict resolution over the affected state subset before acceptance
//! ([`GaussMatrix-SPECS.pdf`], §III-D). The specification attacks this on three
//! fronts: parallel signature verification, a **resolved-state cache** keyed by
//! the set of conflicting state-event identifiers, and recursive relation
//! gathering.
//!
//! This crate provides the deterministic state-resolution engine:
//!
//! * [`partition`] — the state-resolution-v2 split of a set of input state maps
//!   into an *unconflicted* map (every map agrees) and a *conflicted* map (the
//!   maps disagree).
//! * [`reverse_topological_power_sort`] and [`mainline_ordering`] — the two
//!   deterministic orderings the conflict-resolution pass applies.
//! * [`iterative_auth_checks`] with the [`AuthRules`] gates ([`CreateRules`],
//!   [`PowerLevelRules`], [`PowerLevelMutationRules`], [`MembershipRules`],
//!   composed via [`AllOf`]) — the room-version authorisation rules.
//! * [`ResolvedStateCache`] — memoises resolution keyed by the conflicting
//!   state-event ids (front two of the specification).
//! * [`resolve`] — the full two-pass state-resolution-v2 algorithm composing all
//!   of the above.
//!
//! ## Scope
//!
//! The engine is **pure with respect to storage**, so its outputs are
//! deterministic and unit-testable. The authorisation rules cover the
//! common-case room-version semantics (create, power-level send and mutation,
//! and the membership transitions including the create-room bootstrap join and
//! knock); restricted joins and third-party invites remain to be added.
//!
//! Event identifiers and state keys are modelled as owned strings here so the
//! engine stays dependency-free; the `gm-api` crate adapts real Matrix event
//! content into the [`Event`] and [`PowerLevels`] models.
//!
//! [`GaussMatrix-SPECS.pdf`]: ../../GaussMatrix-SPECS.pdf

#![forbid(unsafe_code)]

use std::collections::BTreeSet;

mod auth;
mod cache;
mod event;
mod order;
mod power;
mod state_map;
#[cfg(test)]
mod tests;

pub use self::{
	auth::{
		AllOf, AuthRules, CreateRules, MembershipRules, PowerLevelMutationRules, PowerLevelRules,
		iterative_auth_checks,
	},
	cache::ResolvedStateCache,
	event::Event,
	order::{auth_difference, mainline_ordering, reverse_topological_power_sort},
	power::PowerLevels,
	state_map::{
		ConflictedState, EventId, EventStore, Partitioned, StateKey, StateMap,
		conflicting_event_ids, partition,
	},
};

/// Resolve a set of conflicting state forks into a single state map using the
/// state-resolution-v2 algorithm, memoising via the [`ResolvedStateCache`].
///
/// The unconflicted slots (every fork agrees) are taken as-is; if there is no
/// conflict that is the whole result. Otherwise the conflicted events, together
/// with the [`auth_difference`] of the forks' auth chains, are resolved in two
/// passes against the unconflicted base:
///
/// 1. the *control* events (power levels, join rules, membership) are
///    [`reverse_topological_power_sort`]ed and run through
///    [`iterative_auth_checks`];
/// 2. the remaining events are [`mainline_ordering`]ed by the resolved
///    power-levels event and run through [`iterative_auth_checks`] against the
///    running result.
///
/// The resolution of the conflicted slots is cached by the conflicting event-id
/// set and merged over the unconflicted state.
///
/// `store` must contain every event referenced by `forks` and `auth_chains`;
/// ids absent from it are skipped.
pub fn resolve<E, R>(
	forks: &[StateMap],
	auth_chains: &[BTreeSet<EventId>],
	store: &EventStore<E>,
	rules: &R,
	cache: &mut ResolvedStateCache,
) -> StateMap
where
	E: Event,
	R: AuthRules<E>,
{
	let Partitioned { unconflicted, conflicted } = partition(forks);
	if conflicted.is_empty() {
		return unconflicted;
	}

	let cache_key = conflicting_event_ids(&conflicted);
	if let Some(resolved) = cache.get(&cache_key) {
		return merged(&unconflicted, resolved);
	}

	// The full conflicted set: the disputed events plus the auth difference.
	let mut full: BTreeSet<EventId> = cache_key.clone();
	full.extend(auth_difference(auth_chains));

	// Split into control events (power levels / join rules / membership) and the
	// rest; only events present in the store can be resolved.
	let (control, others): (Vec<EventId>, Vec<EventId>) = full
		.into_iter()
		.filter(|id| store.contains_key(id))
		.partition(|id| store.get(id).is_some_and(is_control_event));

	// Pass one: power-order the control events, auth-check against unconflicted.
	let ordered_control = reverse_topological_power_sort(&control, store);
	let mut partial = iterative_auth_checks(&ordered_control, unconflicted.clone(), store, rules);

	// Pass two: mainline-order the rest by the resolved power-levels event,
	// auth-check against the running result.
	let resolved_power_levels = partial
		.get(&("m.room.power_levels".to_owned(), String::new()))
		.cloned();
	let ordered_others = mainline_ordering(resolved_power_levels.as_deref(), &others, store);
	partial = iterative_auth_checks(&ordered_others, partial, store, rules);

	// The resolution of the conflicted slots, cached and merged over the
	// unconflicted state.
	let resolved: StateMap = conflicted
		.keys()
		.filter_map(|slot| partial.get(slot).map(|id| (slot.clone(), id.clone())))
		.collect();
	cache.insert(cache_key, resolved.clone());

	merged(&unconflicted, &resolved)
}

/// Merge the resolved conflicted slots over the unconflicted state.
fn merged(unconflicted: &StateMap, resolved: &StateMap) -> StateMap {
	let mut out = unconflicted.clone();
	out.extend(resolved.iter().map(|(slot, id)| (slot.clone(), id.clone())));
	out
}

/// Whether an event is a control event — one of the types that govern
/// authorisation and are resolved in the first pass.
fn is_control_event<E: Event>(event: &E) -> bool {
	matches!(event.event_type(), "m.room.power_levels" | "m.room.join_rules" | "m.room.member")
}
