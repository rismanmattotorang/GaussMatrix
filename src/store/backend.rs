//! The backend-agnostic key/value trait and the atomic write batch.

use crate::{Domain, Result};

/// A single `(key, value)` pair yielded by a scan, parameterised over the
/// backend's value handle.
pub type Entry<V> = (Box<[u8]>, V);

/// A single mutation within a [`WriteBatch`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
	/// Insert or overwrite `key` with `val` in `domain`.
	Put {
		/// Target domain (column family).
		domain: Domain,
		/// Record key.
		key: Box<[u8]>,
		/// Record value.
		val: Box<[u8]>,
	},

	/// Remove `key` from `domain` if present.
	Delete {
		/// Target domain (column family).
		domain: Domain,
		/// Record key.
		key: Box<[u8]>,
	},
}

/// An ordered set of mutations applied to the store **atomically**.
///
/// The specification requires that "writes are batched per request into a
/// single atomic commit so that an accepted event and its state-delta become
/// visible together". A backend MUST apply all operations in a batch or none of
/// them, and a concurrent reader MUST never observe a partial batch.
#[derive(Clone, Debug, Default)]
pub struct WriteBatch {
	ops: Vec<Op>,
}

impl WriteBatch {
	/// Create an empty batch.
	#[must_use]
	pub fn new() -> Self { Self::default() }

	/// Queue a put of `key` → `val` in `domain`.
	pub fn put<K, V>(&mut self, domain: Domain, key: K, val: V) -> &mut Self
	where
		K: Into<Box<[u8]>>,
		V: Into<Box<[u8]>>,
	{
		self.ops
			.push(Op::Put { domain, key: key.into(), val: val.into() });
		self
	}

	/// Queue a delete of `key` from `domain`.
	pub fn delete<K>(&mut self, domain: Domain, key: K) -> &mut Self
	where
		K: Into<Box<[u8]>>,
	{
		self.ops
			.push(Op::Delete { domain, key: key.into() });
		self
	}

	/// The queued operations, in insertion order.
	#[must_use]
	pub fn ops(&self) -> &[Op] { &self.ops }

	/// Number of queued operations.
	#[must_use]
	pub fn len(&self) -> usize { self.ops.len() }

	/// Whether the batch has no operations.
	#[must_use]
	pub fn is_empty(&self) -> bool { self.ops.is_empty() }
}

/// A pluggable key/value backend over the fixed set of [`Domain`]s.
///
/// This is the single seam between the GaussMatrix service core and the storage
/// engine. Implementors include the in-memory [`MemBackend`](crate::MemBackend)
/// reference, the production tuned-RocksDB engine, and (Phase 2) a distributed
/// KV backend behind the room-sharding layer.
pub trait KvBackend: Send + Sync + 'static {
	/// The value handle returned by reads.
	///
	/// Backends expose their cheapest representation here to honour the
	/// "zero-copy where the backend permits" rule: RocksDB yields a pinned
	/// slice, the in-memory backend an `Arc<[u8]>`.
	type Value: AsRef<[u8]> + Send + Sync;

	/// Fetch the value for `key` in `domain`, if present.
	fn get(&self, domain: Domain, key: &[u8]) -> Result<Option<Self::Value>>;

	/// Whether `domain` contains `key`.
	fn contains(&self, domain: Domain, key: &[u8]) -> Result<bool> {
		Ok(self.get(domain, key)?.is_some())
	}

	/// Snapshot every `(key, value)` in `domain` whose key starts with
	/// `prefix`, in ascending key order.
	///
	/// A `prefix` of `&[]` scans the whole domain. The reference contract
	/// returns an owned snapshot; streaming iteration is a backend refinement.
	fn prefix_scan(
		&self,
		domain: Domain,
		prefix: &[u8],
	) -> Result<Vec<Entry<Self::Value>>>;

	/// Apply `batch` atomically: all operations commit, or none do.
	fn commit(&self, batch: WriteBatch) -> Result<()>;
}
