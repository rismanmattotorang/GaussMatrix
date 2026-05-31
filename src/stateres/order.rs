//! Deterministic ordering primitives for state resolution v2.
//!
//! These are the storage-pure ordering steps the conflict-resolution pass
//! composes: the [`auth_difference`] of the input forks, and the
//! [`reverse_topological_power_sort`] of the conflicted control events. The
//! room-version authorisation *checks* that consume this ordering are the next
//! increment; they do not change these orderings.

use std::{
	cmp::Ordering,
	collections::{BTreeMap, BTreeSet, BinaryHeap},
};

use crate::{event::Event, state_map::EventId};

/// Events that depend on a given auth event (the reverse of the auth edges),
/// used to drive Kahn's algorithm.
type Dependents = BTreeMap<EventId, Vec<EventId>>;

/// The auth difference of a set of state forks: every event id that appears in
/// at least one fork's auth chain but not in all of them.
///
/// Each fork is the set of event ids in one conflicting state's auth chains.
/// The difference is the union minus the intersection — the events whose
/// authorisation the forks do not already agree on.
#[must_use]
pub fn auth_difference(forks: &[BTreeSet<EventId>]) -> BTreeSet<EventId> {
	let mut forks = forks.iter();
	let Some(first) = forks.next() else {
		return BTreeSet::new();
	};

	let union: BTreeSet<EventId> = std::iter::once(first)
		.chain(forks.clone())
		.flatten()
		.cloned()
		.collect();

	let mut intersection = first.clone();
	for fork in forks {
		intersection = intersection.intersection(fork).cloned().collect();
	}

	union.difference(&intersection).cloned().collect()
}

/// The heap key implementing the reverse-topological-power comparison: an event
/// is ordered before another when it has a higher sender power level, then an
/// earlier `origin_server_ts`, then a lexicographically smaller id. The `Ord`
/// impl is arranged so the greatest key is the one to emit first.
#[derive(Clone, PartialEq, Eq)]
struct PowerKey {
	power: i64,
	ts: u64,
	id: EventId,
}

impl Ord for PowerKey {
	fn cmp(&self, other: &Self) -> Ordering {
		// Higher power is "greater"; then lower ts is "greater"; then lower id.
		self.power
			.cmp(&other.power)
			.then_with(|| other.ts.cmp(&self.ts))
			.then_with(|| other.id.cmp(&self.id))
	}
}

impl PartialOrd for PowerKey {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

/// Order `events` into reverse topological power order: each event appears after
/// the auth events that are also in `events`, and among events whose in-set auth
/// dependencies are already satisfied, the one with the highest sender power
/// level (ties: earliest `origin_server_ts`, then smallest id) is emitted first.
///
/// Auth events referenced but not present in `events` are ignored — the order is
/// over the given subset only. The auth graph is assumed acyclic, as Matrix auth
/// chains are.
#[must_use]
pub fn reverse_topological_power_sort<E: Event>(events: &[E]) -> Vec<EventId> {
	let present: BTreeSet<&str> = events.iter().map(Event::event_id).collect();

	let mut key_of: BTreeMap<EventId, PowerKey> = BTreeMap::new();
	let mut remaining: BTreeMap<EventId, usize> = BTreeMap::new();
	let mut dependents: Dependents = BTreeMap::new();

	for event in events {
		let id = event.event_id().to_owned();
		let deps: BTreeSet<EventId> = event
			.auth_event_ids()
			.iter()
			.filter(|auth| present.contains(auth.as_str()))
			.cloned()
			.collect();

		key_of.insert(id.clone(), PowerKey {
			power: event.power_level(),
			ts: event.origin_server_ts(),
			id: id.clone(),
		});
		remaining.insert(id.clone(), deps.len());
		for dep in deps {
			dependents.entry(dep).or_default().push(id.clone());
		}
	}

	// Seed the heap with events that have no in-set auth dependencies.
	let mut ready: BinaryHeap<PowerKey> = remaining
		.iter()
		.filter(|(_, count)| **count == 0)
		.filter_map(|(id, _)| key_of.get(id).cloned())
		.collect();

	let mut ordered = Vec::with_capacity(events.len());
	while let Some(key) = ready.pop() {
		let id = key.id.clone();
		ordered.push(id.clone());

		if let Some(children) = dependents.get(&id) {
			for child in children {
				if let Some(count) = remaining.get_mut(child) {
					*count = count.saturating_sub(1);
					if *count == 0
						&& let Some(child_key) = key_of.get(child)
					{
						ready.push(child_key.clone());
					}
				}
			}
		}
	}

	ordered
}
