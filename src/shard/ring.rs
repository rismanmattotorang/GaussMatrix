//! The consistent-hash placement ring.

use std::collections::{BTreeMap, BTreeSet};

/// A shard identifier (e.g. a worker name or address).
pub type ShardId = String;

/// A room whose owning shard changes between two ring states.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reassignment {
	/// The affected room.
	pub room: String,
	/// The shard that owned it before.
	pub from: ShardId,
	/// The shard that owns it after.
	pub to: ShardId,
}

/// Virtual nodes per shard. More points spread a shard's keyspace more evenly,
/// keeping the partitions balanced and the reassignment on membership change
/// close to its `1/N` ideal.
const VIRTUAL_NODES: u32 = 128;

/// FNV-1a 64-bit offset basis and prime — a fixed, portable hash so every
/// front-end places a room identically without coordination.
const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

/// A consistent-hash ring mapping rooms to the shard that owns them.
#[derive(Clone, Debug, Default)]
pub struct ShardRing {
	/// Virtual-node hash → owning shard, ordered for ring lookups.
	ring: BTreeMap<u64, ShardId>,
	/// The distinct shards in the ring.
	shards: BTreeSet<ShardId>,
}

impl ShardRing {
	/// An empty ring.
	#[must_use]
	pub fn new() -> Self { Self::default() }

	/// Add a shard, placing its virtual nodes on the ring. Idempotent.
	pub fn add_shard(&mut self, shard: &str) {
		if !self.shards.insert(shard.to_owned()) {
			return;
		}
		for vnode in 0..VIRTUAL_NODES {
			self.ring.insert(virtual_node_hash(shard, vnode), shard.to_owned());
		}
	}

	/// Remove a shard and its virtual nodes. Rooms it owned are reassigned to
	/// the next shard on the ring; rooms owned by other shards are unaffected.
	pub fn remove_shard(&mut self, shard: &str) {
		if !self.shards.remove(shard) {
			return;
		}
		self.ring.retain(|_, owner| owner != shard);
	}

	/// The shard that owns `room_id`, or `None` if the ring is empty.
	#[must_use]
	pub fn shard_for(&self, room_id: &str) -> Option<&str> {
		let point = hash(room_id.as_bytes());
		self.ring
			.range(point..)
			.next()
			.or_else(|| self.ring.iter().next())
			.map(|(_, shard)| shard.as_str())
	}

	/// The shards currently in the ring.
	pub fn shards(&self) -> impl Iterator<Item = &str> + '_ {
		self.shards.iter().map(String::as_str)
	}

	/// The reassignments needed to move the given `rooms` from this ring's
	/// placement to `target`'s — the work a coordination service warms before
	/// cut-over when a shard is added or drained. Only rooms whose owner changes
	/// are returned.
	#[must_use]
	pub fn reassignments(&self, target: &Self, rooms: &[String]) -> Vec<Reassignment> {
		rooms
			.iter()
			.filter_map(|room| {
				let from = self.shard_for(room)?;
				let to = target.shard_for(room)?;
				(from != to).then(|| Reassignment {
					room: room.clone(),
					from: from.to_owned(),
					to: to.to_owned(),
				})
			})
			.collect()
	}

	/// The number of shards.
	#[must_use]
	pub fn len(&self) -> usize { self.shards.len() }

	/// Whether the ring has no shards.
	#[must_use]
	pub fn is_empty(&self) -> bool { self.shards.is_empty() }
}

/// The ring point for a shard's `vnode`-th virtual node.
fn virtual_node_hash(shard: &str, vnode: u32) -> u64 {
	let mut key = shard.as_bytes().to_vec();
	key.push(b'#');
	key.extend_from_slice(&vnode.to_be_bytes());
	hash(&key)
}

/// A uniform 64-bit hash: FNV-1a followed by a splitmix64 finalizer. The
/// finalizer's avalanche spreads the ring points evenly, which FNV-1a alone
/// does not — keeping partitions balanced and reassignment near its `1/N`
/// ideal.
fn hash(data: &[u8]) -> u64 {
	let mut state = FNV_OFFSET;
	for &byte in data {
		state ^= u64::from(byte);
		state = state.wrapping_mul(FNV_PRIME);
	}

	finalize(state)
}

/// splitmix64 finalizing mix.
fn finalize(mut z: u64) -> u64 {
	z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
	z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
	z ^ (z >> 31)
}
