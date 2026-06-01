//! Tests for consistent-hash room placement.

use std::collections::BTreeMap;

use crate::{Reassignment, ShardRing};

fn ring(shards: &[&str]) -> ShardRing {
	let mut ring = ShardRing::new();
	for shard in shards {
		ring.add_shard(shard);
	}
	ring
}

fn rooms(count: usize) -> Vec<String> {
	(0..count).map(|i| format!("!room{i}:example.org")).collect()
}

#[test]
fn empty_ring_places_nothing() {
	assert!(ShardRing::new().shard_for("!r:x").is_none());
	assert!(ShardRing::new().is_empty());
}

#[test]
fn placement_is_deterministic() {
	let ring = ring(&["a", "b", "c"]);
	assert_eq!(ring.shard_for("!room:x"), ring.shard_for("!room:x"));
	assert!(ring.shard_for("!room:x").is_some());
}

#[test]
fn add_shard_is_idempotent() {
	let mut ring = ShardRing::new();
	ring.add_shard("a");
	ring.add_shard("a");
	assert_eq!(ring.len(), 1);
}

#[test]
fn every_room_maps_to_a_member_shard() {
	let ring = ring(&["a", "b", "c"]);
	for room in rooms(500) {
		let owner = ring.shard_for(&room).unwrap();
		assert!(ring.shards().any(|s| s == owner));
	}
}

#[test]
fn placement_is_reasonably_balanced() {
	let ring = ring(&["a", "b", "c", "d"]);
	let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
	let all = rooms(4000);
	for room in &all {
		*counts.entry(ring.shard_for(room).unwrap()).or_default() += 1;
	}
	// With 128 vnodes/shard each of the 4 shards should hold a healthy share;
	// require every shard between 10% and 40% (ideal 25%).
	for (shard, count) in &counts {
		assert!(
			(400..=1600).contains(count),
			"shard {shard} holds {count}/4000 — outside the balanced range"
		);
	}
	assert_eq!(counts.len(), 4, "all shards should own rooms");
}

#[test]
fn adding_a_shard_reassigns_only_a_minority() {
	let all = rooms(4000);
	let before = ring(&["a", "b", "c"]);
	let placement: Vec<String> =
		all.iter().map(|r| before.shard_for(r).unwrap().to_owned()).collect();

	let mut after = before;
	after.add_shard("d");

	let moved = all
		.iter()
		.zip(&placement)
		.filter(|(room, old)| after.shard_for(room).unwrap() != old.as_str())
		.count();

	// Consistent hashing: far fewer than half move (ideal ~1/4 onto the new
	// shard); only rooms that land on the new shard's arcs are reassigned.
	assert!(moved < all.len() / 2, "{moved}/4000 rooms moved — not consistent");
}

#[test]
fn removing_a_shard_only_moves_its_rooms() {
	let all = rooms(2000);
	let before = ring(&["a", "b", "c"]);
	let placement: Vec<String> =
		all.iter().map(|r| before.shard_for(r).unwrap().to_owned()).collect();

	let mut after = before;
	after.remove_shard("b");
	assert_eq!(after.len(), 2);

	for (room, old) in all.iter().zip(&placement) {
		let new = after.shard_for(room).unwrap();
		if old == "b" {
			assert!(new == "a" || new == "c", "removed shard's rooms reassign elsewhere");
		} else {
			assert_eq!(new, old.as_str(), "rooms not on the removed shard keep their owner");
		}
	}
}

#[test]
fn reassignments_on_add_target_only_the_new_shard() {
	let all = rooms(2000);
	let before = ring(&["a", "b", "c"]);
	let after = ring(&["a", "b", "c", "d"]);

	let moves: Vec<Reassignment> = before.reassignments(&after, &all);
	assert!(!moves.is_empty());
	for mv in &moves {
		// Adding a shard only pulls rooms onto the new shard.
		assert_eq!(mv.to, "d", "rooms only move onto the added shard");
		assert!(matches!(mv.from.as_str(), "a" | "b" | "c"));
	}
}

#[test]
fn reassignments_on_drain_come_only_from_the_removed_shard() {
	let all = rooms(2000);
	let before = ring(&["a", "b", "c"]);
	let after = ring(&["a", "c"]);

	let moves = before.reassignments(&after, &all);
	assert!(!moves.is_empty());
	for mv in &moves {
		assert_eq!(mv.from, "b", "only the drained shard's rooms move");
		assert!(matches!(mv.to.as_str(), "a" | "c"));
	}
}

#[test]
fn reassignments_empty_when_ring_unchanged() {
	let all = rooms(200);
	let ring = ring(&["a", "b"]);
	assert!(ring.reassignments(&ring, &all).is_empty());
}
