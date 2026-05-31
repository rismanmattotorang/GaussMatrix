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
	state_map::{EventId, EventStore, StateMap},
};

/// The room-version authorisation rules: decide whether `event` is authorised
/// against the partially-resolved `state`.
pub trait AuthRules<E: Event> {
	/// Whether `event` is authorised given the state resolved so far.
	fn is_authorized(&self, event: &E, state: &StateMap) -> bool;
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

		if rules.is_authorized(event, &state) {
			let slot = (event.event_type().to_owned(), event.state_key().to_owned());
			state.insert(slot, id.clone());
		}
	}

	state
}
