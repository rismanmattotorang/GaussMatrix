//! Room placement across worker shards (SPECS §V).
//!
//! A thin front-end seam over gm-shard's consistent-hash [`ShardRing`]: given
//! the set of worker shards, every front-end places a room on the same shard
//! without coordination, and adding or draining a shard moves only its `1/N`
//! share of rooms. In the single-node profile the ring carries one shard — this
//! server — so every room places locally; the identical helper scales out to a
//! multi-worker deployment by seeding the ring with more shards.

use gaussmatrix_core::debug;
use gm_shard::ShardRing;
use ruma::RoomId;

/// Consistent-hash placement of rooms onto worker shards.
pub struct RoomPlacement {
	ring: ShardRing,
}

impl RoomPlacement {
	/// Build a placement over `shards`. The mapping is deterministic in the
	/// shard set; insertion order does not affect it.
	#[must_use]
	pub fn new(shards: &[String]) -> Self {
		let mut ring = ShardRing::new();
		for shard in shards {
			ring.add_shard(shard);
		}
		Self { ring }
	}

	/// A single-node placement: every room places on the local server.
	#[must_use]
	pub fn local(server_name: &str) -> Self {
		Self::new(std::slice::from_ref(&server_name.to_owned()))
	}

	/// The shard that owns `room_id`, or `None` if no shards are configured.
	#[must_use]
	pub fn shard_for(&self, room_id: &RoomId) -> Option<&str> {
		self.ring.shard_for(room_id.as_str())
	}

	/// Whether `room_id` is owned by `shard`.
	#[must_use]
	pub fn is_owned_by(&self, room_id: &RoomId, shard: &str) -> bool {
		self.shard_for(room_id) == Some(shard)
	}

	/// The number of shards in the ring.
	#[must_use]
	pub fn len(&self) -> usize { self.ring.len() }

	/// Whether the ring has no shards.
	#[must_use]
	pub fn is_empty(&self) -> bool { self.ring.is_empty() }
}

/// Install the front-end placement ring for the local server and log readiness.
/// This is the single-node seam: the ring is local-only here, and grows to a
/// multi-worker ring without touching call sites that consult [`RoomPlacement`].
pub fn init_local(server_name: &str) -> RoomPlacement {
	let placement = RoomPlacement::local(server_name);
	debug!(shards = placement.len(), "Room placement ring ready (single-node profile)");
	placement
}

#[cfg(test)]
mod tests {
	use ruma::room_id;

	use super::RoomPlacement;

	#[test]
	fn local_placement_owns_every_room() {
		let placement = RoomPlacement::local("gauss.example.org");
		assert_eq!(placement.len(), 1);
		assert!(!placement.is_empty());

		let room = room_id!("!abcdef:gauss.example.org");
		assert_eq!(placement.shard_for(room), Some("gauss.example.org"));
		assert!(placement.is_owned_by(room, "gauss.example.org"));
		assert!(!placement.is_owned_by(room, "other.example.org"));
	}

	#[test]
	fn empty_ring_places_nothing() {
		let placement = RoomPlacement::new(&[]);
		assert!(placement.is_empty());
		assert_eq!(placement.shard_for(room_id!("!x:gauss.example.org")), None);
	}

	#[test]
	fn placement_is_deterministic_in_shard_set() {
		let forward =
			RoomPlacement::new(&["a".to_owned(), "b".to_owned(), "c".to_owned()]);
		let reverse =
			RoomPlacement::new(&["c".to_owned(), "b".to_owned(), "a".to_owned()]);

		let room = room_id!("!room:gauss.example.org");
		assert_eq!(forward.shard_for(room), reverse.shard_for(room));
	}
}
