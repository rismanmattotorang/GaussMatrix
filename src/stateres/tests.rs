//! Unit tests for partitioning, the resolved-state cache, and resolution.

use std::cell::Cell;

use crate::{
	ConflictedState, ResolvedStateCache, StateKey, StateMap, conflicting_event_ids, partition,
	resolve,
};

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
	let mk = |id: &str| -> std::collections::BTreeSet<String> {
		let mut s = std::collections::BTreeSet::new();
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
	let mut k = std::collections::BTreeSet::new();
	k.insert("$x".to_owned());
	cache.insert(k.clone(), StateMap::new());
	assert!(cache.get(&k).is_none());
	assert!(cache.is_empty());
}

#[test]
fn cache_reinsert_refreshes_value_without_duplicate_slot() {
	let mut cache = ResolvedStateCache::new(8);
	let mut k = std::collections::BTreeSet::new();
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
