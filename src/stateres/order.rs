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

use crate::{
	event::Event,
	state_map::{EventId, EventStore},
};

/// Events that depend on a given auth event (the reverse of the auth edges),
/// used to drive Kahn's algorithm.
type Dependents = BTreeMap<EventId, Vec<EventId>>;

/// Mainline order index: each power-levels event on the mainline mapped to its
/// position, counting from the mainline root (0) toward the resolved event.
type MainlineIndex = BTreeMap<EventId, usize>;

/// The event type whose chain through auth events forms the mainline.
const POWER_LEVELS_TYPE: &str = "m.room.power_levels";

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

/// Order the events named by `ids` into reverse topological power order: each
/// event appears after the auth events that are also in the set, and among
/// events whose in-set auth dependencies are already satisfied, the one with the
/// highest sender power level (ties: earliest `origin_server_ts`, then smallest
/// id) is emitted first.
///
/// Ids absent from `store`, and auth events outside the set, are ignored — the
/// order is over the present subset only. The auth graph is assumed acyclic, as
/// Matrix auth chains are.
#[must_use]
pub fn reverse_topological_power_sort<E: Event>(
	ids: &[EventId],
	store: &EventStore<E>,
) -> Vec<EventId> {
	let events: Vec<&E> = ids.iter().filter_map(|id| store.get(id)).collect();
	let present: BTreeSet<&str> = events.iter().map(|event| event.event_id()).collect();

	let mut key_of: BTreeMap<EventId, PowerKey> = BTreeMap::new();
	let mut remaining: BTreeMap<EventId, usize> = BTreeMap::new();
	let mut dependents: Dependents = BTreeMap::new();

	for event in &events {
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

/// Order `to_order` by mainline ordering against the resolved power-levels event.
///
/// The *mainline* is the chain of power-levels events reached by following auth
/// events from `resolved_power_levels` back to the room's first power-levels
/// event. Each event to order is assigned the mainline position of the closest
/// power-levels event in its auth chain (its *mainline depth*); events are then
/// ordered ascending by `(mainline_depth, origin_server_ts, event_id)`, so
/// events anchored closer to the mainline root come first.
///
/// `store` must contain every event referenced (those being ordered and the
/// power-levels events along their and the mainline's auth chains). Events with
/// no power-levels ancestor on the mainline take depth zero.
#[must_use]
pub fn mainline_ordering<E: Event>(
	resolved_power_levels: Option<&str>,
	to_order: &[EventId],
	store: &EventStore<E>,
) -> Vec<EventId> {
	// Build the mainline by following power-levels auth links from the resolved
	// event, then index it from the root (0) toward the resolved event.
	let mut mainline: Vec<EventId> = Vec::new();
	let mut cursor = resolved_power_levels.map(str::to_owned);
	while let Some(id) = cursor {
		cursor = power_levels_in_auth(&id, store);
		mainline.push(id);
	}

	let mut index: MainlineIndex = BTreeMap::new();
	for (position, id) in mainline.iter().rev().enumerate() {
		index.insert(id.clone(), position);
	}

	let mut keyed: Vec<(usize, u64, EventId)> = to_order
		.iter()
		.map(|id| {
			let depth = mainline_depth(id, store, &index);
			let ts = store.get(id).map_or(0, Event::origin_server_ts);
			(depth, ts, id.clone())
		})
		.collect();

	keyed.sort_unstable();
	keyed.into_iter().map(|(_, _, id)| id).collect()
}

/// The mainline depth of `start`: the mainline position of the closest
/// power-levels event reachable by following power-levels auth links, or zero if
/// none is on the mainline.
fn mainline_depth<E: Event>(
	start: &str,
	store: &EventStore<E>,
	index: &MainlineIndex,
) -> usize {
	let mut cursor = Some(start.to_owned());
	while let Some(id) = cursor {
		if let Some(position) = index.get(&id) {
			return *position;
		}
		cursor = power_levels_in_auth(&id, store);
	}

	0
}

/// The power-levels event among `id`'s auth events, if any.
fn power_levels_in_auth<E: Event>(id: &str, store: &EventStore<E>) -> Option<EventId> {
	let event = store.get(id)?;
	event
		.auth_event_ids()
		.iter()
		.find(|auth| {
			store
				.get(*auth)
				.is_some_and(|e| e.event_type() == POWER_LEVELS_TYPE)
		})
		.cloned()
}
