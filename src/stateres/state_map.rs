//! State-map types and the conflict-partitioning step.

use std::collections::{BTreeMap, BTreeSet};

/// A Matrix event identifier (e.g. `$abc:example.org`).
pub type EventId = String;

/// A by-id lookup of the events relevant to a resolution. Callers populate this
/// with the conflicted events and the auth events along their chains.
pub type EventStore<E> = BTreeMap<EventId, E>;

/// A state key: the `(event_type, state_key)` pair that names one slot of room
/// state (for example `("m.room.member", "@alice:example.org")`).
pub type StateKey = (String, String);

/// A resolved room-state mapping: each state slot points at one event.
pub type StateMap = BTreeMap<StateKey, EventId>;

/// The conflicted subset: each slot maps to the set of distinct events the input
/// state maps disagree over.
pub type ConflictedState = BTreeMap<StateKey, BTreeSet<EventId>>;

/// The result of [`partition`]: the agreed-upon state and the disputed state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Partitioned {
	/// Slots on which every input state map agrees (exactly one distinct event).
	pub unconflicted: StateMap,

	/// Slots on which the input state maps disagree (more than one distinct
	/// event).
	pub conflicted: ConflictedState,
}

/// Partition a set of input state maps into unconflicted and conflicted state,
/// following the state-resolution-v2 definition.
///
/// For each state slot, the distinct event ids assigned across all input maps
/// are collected. A slot with exactly one distinct event id is *unconflicted*
/// (maps that omit the slot do not create a conflict); a slot with more than one
/// is *conflicted*.
#[must_use]
pub fn partition(states: &[StateMap]) -> Partitioned {
	let mut grouped: BTreeMap<StateKey, BTreeSet<EventId>> = BTreeMap::new();
	for state in states {
		for (key, event_id) in state {
			grouped
				.entry(key.clone())
				.or_default()
				.insert(event_id.clone());
		}
	}

	let mut out = Partitioned::default();
	for (key, mut event_ids) in grouped {
		if event_ids.len() == 1 {
			// Exactly one distinct event: take it as unconflicted.
			if let Some(event_id) = event_ids.pop_first() {
				out.unconflicted.insert(key, event_id);
			}
		} else {
			out.conflicted.insert(key, event_ids);
		}
	}

	out
}

/// The flattened set of every event id appearing in a conflicted state. This is
/// the key under which a resolution result is memoised.
#[must_use]
pub fn conflicting_event_ids(conflicted: &ConflictedState) -> BTreeSet<EventId> {
	conflicted
		.values()
		.flat_map(|ids| ids.iter().cloned())
		.collect()
}
