//! Unit tests for partitioning, the resolved-state cache, and resolution.

use std::{cell::Cell, collections::BTreeSet};

use crate::{
	ConflictedState, Event, EventId, ResolvedStateCache, StateKey, StateMap, auth_difference,
	conflicting_event_ids, partition, resolve, reverse_topological_power_sort,
};

/// A test event carrying just the fields the ordering needs.
struct TestEvent {
	id: EventId,
	power: i64,
	ts: u64,
	auth: Vec<EventId>,
}

impl TestEvent {
	fn new(id: &str, power: i64, ts: u64, auth: &[&str]) -> Self {
		Self {
			id: id.to_owned(),
			power,
			ts,
			auth: auth.iter().map(|a| (*a).to_owned()).collect(),
		}
	}
}

impl Event for TestEvent {
	fn event_id(&self) -> &str { &self.id }

	fn power_level(&self) -> i64 { self.power }

	fn origin_server_ts(&self) -> u64 { self.ts }

	fn auth_event_ids(&self) -> &[EventId] { &self.auth }
}

fn id_set(ids: &[&str]) -> BTreeSet<EventId> {
	ids.iter().map(|s| (*s).to_owned()).collect()
}

fn key(event_type: &str, state_key: &str) -> StateKey {
	(event_type.to_owned(), state_key.to_owned())
}

/// Build a state map from `(type, state_key, event_id)` triples.
fn state(pairs: &[(&str, &str, &str)]) -> StateMap {
	pairs
		.iter()
		.map(|(t, s, e)| (key(t, s), (*e).to_owned()))
		.collect()
}

/// A deterministic stand-in for the room-version auth ordering: pick the
/// lexicographically smallest event id per conflicted slot.
fn order_smallest(conflicted: &ConflictedState) -> StateMap {
	conflicted
		.iter()
		.map(|(k, ids)| (k.clone(), ids.iter().next().cloned().unwrap()))
		.collect()
}

#[test]
fn partition_all_agree_is_unconflicted() {
	let a = state(&[("m.room.name", "", "$n1"), ("m.room.topic", "", "$t1")]);
	let b = a.clone();
	let p = partition(&[a, b]);
	assert_eq!(p.unconflicted.len(), 2);
	assert!(p.conflicted.is_empty());
	assert_eq!(p.unconflicted.get(&key("m.room.name", "")).map(String::as_str), Some("$n1"));
}

#[test]
fn partition_disagreement_is_conflicted() {
	let a = state(&[("m.room.name", "", "$n1")]);
	let b = state(&[("m.room.name", "", "$n2")]);
	let p = partition(&[a, b]);
	assert!(p.unconflicted.is_empty());
	assert_eq!(p.conflicted.len(), 1);
	assert_eq!(p.conflicted[&key("m.room.name", "")].len(), 2);
}

#[test]
fn partition_absent_in_some_is_still_unconflicted() {
	// A slot present in only one map, with a single value, is not a conflict.
	let a = state(&[("m.room.name", "", "$n1"), ("m.room.topic", "", "$t1")]);
	let b = state(&[("m.room.name", "", "$n1")]);
	let p = partition(&[a, b]);
	assert!(p.conflicted.is_empty());
	assert_eq!(p.unconflicted.len(), 2);
}

#[test]
fn conflicting_event_ids_flattens_all_disputed_events() {
	let a = state(&[("m.room.name", "", "$n1"), ("m.room.topic", "", "$t1")]);
	let b = state(&[("m.room.name", "", "$n2"), ("m.room.topic", "", "$t2")]);
	let p = partition(&[a, b]);
	let ids = conflicting_event_ids(&p.conflicted);
	assert_eq!(ids.len(), 4);
	assert!(ids.contains("$n1") && ids.contains("$t2"));
}

#[test]
fn cache_insert_and_get() {
	let mut cache = ResolvedStateCache::new(4);
	let k = conflicting_event_ids(&partition(&[state(&[("a", "", "$1")]), state(&[("a", "", "$2")])]).conflicted);
	assert!(cache.get(&k).is_none());
	cache.insert(k.clone(), state(&[("a", "", "$1")]));
	assert_eq!(cache.get(&k).unwrap().get(&key("a", "")).map(String::as_str), Some("$1"));
	assert_eq!(cache.len(), 1);
	assert!(!cache.is_empty());
}

#[test]
fn cache_evicts_least_recently_inserted_over_capacity() {
	let mut cache = ResolvedStateCache::new(2);
	let mk = |id: &str| -> BTreeSet<String> {
		let mut s = BTreeSet::new();
		s.insert(id.to_owned());
		s
	};
	let (a, b, c) = (mk("$a"), mk("$b"), mk("$c"));
	cache.insert(a.clone(), StateMap::new());
	cache.insert(b.clone(), StateMap::new());
	cache.insert(c.clone(), StateMap::new());
	assert_eq!(cache.len(), 2);
	assert!(cache.get(&a).is_none(), "oldest entry should have been evicted");
	assert!(cache.get(&b).is_some());
	assert!(cache.get(&c).is_some());
}

#[test]
fn cache_zero_capacity_disables_caching() {
	let mut cache = ResolvedStateCache::new(0);
	let mut k = BTreeSet::new();
	k.insert("$x".to_owned());
	cache.insert(k.clone(), StateMap::new());
	assert!(cache.get(&k).is_none());
	assert!(cache.is_empty());
}

#[test]
fn cache_reinsert_refreshes_value_without_duplicate_slot() {
	let mut cache = ResolvedStateCache::new(8);
	let mut k = BTreeSet::new();
	k.insert("$x".to_owned());
	cache.insert(k.clone(), state(&[("a", "", "$old")]));
	cache.insert(k.clone(), state(&[("a", "", "$new")]));
	assert_eq!(cache.len(), 1);
	assert_eq!(cache.get(&k).unwrap().get(&key("a", "")).map(String::as_str), Some("$new"));
}

#[test]
fn resolve_without_conflict_returns_unconflicted() {
	let a = state(&[("m.room.name", "", "$n1")]);
	let b = a.clone();
	let mut cache = ResolvedStateCache::new(4);
	let resolved = resolve(&[a, b], &mut cache, order_smallest);
	assert_eq!(resolved.len(), 1);
	assert!(cache.is_empty(), "no conflict means nothing to memoise");
}

#[test]
fn resolve_orders_conflict_and_merges_with_unconflicted() {
	let a = state(&[("m.room.name", "", "$n1"), ("m.room.topic", "", "$shared")]);
	let b = state(&[("m.room.name", "", "$n2"), ("m.room.topic", "", "$shared")]);
	let mut cache = ResolvedStateCache::new(4);
	let resolved = resolve(&[a, b], &mut cache, order_smallest);

	// topic was unconflicted; name was resolved to the smallest id ($n1 < $n2).
	assert_eq!(resolved.get(&key("m.room.topic", "")).map(String::as_str), Some("$shared"));
	assert_eq!(resolved.get(&key("m.room.name", "")).map(String::as_str), Some("$n1"));
	assert_eq!(cache.len(), 1);
}

#[test]
fn resolve_memoises_and_skips_reordering_on_cache_hit() {
	let a = state(&[("m.room.name", "", "$n1")]);
	let b = state(&[("m.room.name", "", "$n2")]);
	let calls = Cell::new(0_u32);
	let mut cache = ResolvedStateCache::new(4);

	let counting = |c: &ConflictedState| {
		calls.set(calls.get().saturating_add(1));
		order_smallest(c)
	};

	let first = resolve(&[a.clone(), b.clone()], &mut cache, counting);
	let second = resolve(&[a, b], &mut cache, counting);

	assert_eq!(first, second);
	assert_eq!(calls.get(), 1, "ordering must run once; the second resolve hits the cache");
}

#[test]
fn auth_difference_is_union_minus_intersection() {
	let forks = [id_set(&["$a", "$b", "$c"]), id_set(&["$b", "$c", "$d"])];
	// $b and $c are shared; $a and $d differ.
	assert_eq!(auth_difference(&forks), id_set(&["$a", "$d"]));
}

#[test]
fn auth_difference_empty_when_all_agree() {
	let forks = [id_set(&["$a", "$b"]), id_set(&["$a", "$b"])];
	assert!(auth_difference(&forks).is_empty());
	assert!(auth_difference(&[]).is_empty());
}

#[test]
fn power_sort_orders_auth_events_before_dependents() {
	// $c authed by $b, $b authed by $a → topological order must be $a, $b, $c.
	let events = [
		TestEvent::new("$c", 100, 1, &["$b"]),
		TestEvent::new("$a", 100, 1, &[]),
		TestEvent::new("$b", 100, 1, &["$a"]),
	];
	assert_eq!(reverse_topological_power_sort(&events), vec!["$a", "$b", "$c"]);
}

#[test]
fn power_sort_breaks_ties_by_power_then_ts_then_id() {
	// Three independent events (no auth deps among them).
	let events = [
		// lower power, should come last among these
		TestEvent::new("$low", 10, 5, &[]),
		// highest power, comes first
		TestEvent::new("$high", 100, 9, &[]),
		// mid power, two with equal power broken by ts then id
		TestEvent::new("$mid_late", 50, 20, &[]),
		TestEvent::new("$mid_early", 50, 10, &[]),
	];
	// highest power first; equal-power pair ordered by earlier ts; lowest last.
	assert_eq!(
		reverse_topological_power_sort(&events),
		vec!["$high", "$mid_early", "$mid_late", "$low"]
	);
}

#[test]
fn power_sort_ignores_auth_events_outside_the_set() {
	// $a's auth event $external is not in the set and must not block emission.
	let events = [TestEvent::new("$a", 50, 1, &["$external"])];
	assert_eq!(reverse_topological_power_sort(&events), vec!["$a"]);
}

#[test]
fn power_sort_equal_keys_broken_by_event_id() {
	let events = [
		TestEvent::new("$b", 50, 10, &[]),
		TestEvent::new("$a", 50, 10, &[]),
	];
	// Identical power and ts → smaller id first.
	assert_eq!(reverse_topological_power_sort(&events), vec!["$a", "$b"]);
}
