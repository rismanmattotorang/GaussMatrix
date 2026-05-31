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
//! This crate provides the deterministic, pure foundation that the parallel
//! engine builds on:
//!
//! * [`partition`] — the state-resolution-v2 split of a set of input state maps
//!   into an *unconflicted* map (every map agrees) and a *conflicted* map (the
//!   maps disagree). This is the input preparation every resolution begins with.
//! * [`ResolvedStateCache`] — memoises the output of resolution keyed by the set
//!   of conflicting state-event ids, so recurrent conflicts are not recomputed
//!   (front two of the specification).
//! * [`resolve`] — the resolution skeleton that ties partitioning and the cache
//!   together, taking the conflicted-state ordering as an injected step.
//!
//! ## Scope
//!
//! The engine is **pure with respect to storage**, so its outputs are
//! deterministic and unit-testable against room-version vectors. The full
//! authorisation-rule ordering for room versions 1–12 (iterative auth checks,
//! power-level and mainline ordering) is the next increment; it plugs into
//! [`resolve`] as the `order_conflicted` step, leaving partitioning and the
//! cache unchanged.
//!
//! Event identifiers and state keys are modelled as owned strings here so the
//! engine stays dependency-free; integration with `ruma`/`gm-api` types happens
//! when the engine is wired into the server.
//!
//! [`GaussMatrix-SPECS.pdf`]: ../../GaussMatrix-SPECS.pdf

#![forbid(unsafe_code)]

mod auth;
mod cache;
mod event;
mod order;
mod state_map;
#[cfg(test)]
mod tests;

pub use self::{
	auth::{AuthRules, iterative_auth_checks},
	cache::ResolvedStateCache,
	event::Event,
	order::{auth_difference, mainline_ordering, reverse_topological_power_sort},
	state_map::{
		ConflictedState, EventId, EventStore, Partitioned, StateKey, StateMap,
		conflicting_event_ids, partition,
	},
};

/// Resolve a set of input state maps into a single state map, memoising via the
/// [`ResolvedStateCache`].
///
/// The unconflicted state is taken as-is. If there is no conflict, that is the
/// whole result. Otherwise the conflicted subset is resolved by the injected
/// `order_conflicted` step (the room-version authorisation ordering), the result
/// is cached by the conflicting event-id set, and the two are merged.
///
/// `order_conflicted` is given the conflicted state and must return exactly one
/// chosen event id per conflicted key.
pub fn resolve<F>(
	states: &[StateMap],
	cache: &mut ResolvedStateCache,
	order_conflicted: F,
) -> StateMap
where
	F: FnOnce(&ConflictedState) -> StateMap,
{
	let Partitioned { mut unconflicted, conflicted } = partition(states);
	if conflicted.is_empty() {
		return unconflicted;
	}

	let key = conflicting_event_ids(&conflicted);
	let resolved = match cache.get(&key) {
		| Some(hit) => hit.clone(),
		| None => {
			let resolved = order_conflicted(&conflicted);
			cache.insert(key, resolved.clone());
			resolved
		},
	};

	unconflicted.extend(resolved);
	unconflicted
}
