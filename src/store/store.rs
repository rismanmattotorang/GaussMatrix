//! The [`Store`] facade over a [`KvBackend`].

use std::sync::Arc;

use crate::{Domain, Entry, KvBackend, Result, WriteBatch};

/// An ergonomic, cloneable handle to a storage backend.
///
/// `Store` is the type the service core holds. It adds convenience helpers over
/// the raw [`KvBackend`] trait (single-key put/delete that commit a one-op
/// atomic batch) while preserving the batch API for multi-domain commits.
///
/// Cloning a `Store` shares the backend.
#[derive(Clone)]
pub struct Store<B: KvBackend> {
	backend: Arc<B>,
}

impl<B: KvBackend> Store<B> {
	/// Wrap an owned backend.
	pub fn new(backend: B) -> Self { Self { backend: Arc::new(backend) } }

	/// Wrap a shared backend handle.
	#[must_use]
	pub fn from_arc(backend: Arc<B>) -> Self { Self { backend } }

	/// Borrow the underlying backend handle.
	#[must_use]
	pub fn backend(&self) -> &Arc<B> { &self.backend }

	/// Fetch the value for `key` in `domain`, if present.
	pub fn get(&self, domain: Domain, key: &[u8]) -> Result<Option<B::Value>> {
		self.backend.get(domain, key)
	}

	/// Whether `domain` contains `key`.
	pub fn contains(&self, domain: Domain, key: &[u8]) -> Result<bool> {
		self.backend.contains(domain, key)
	}

	/// Snapshot every `(key, value)` in `domain` whose key starts with
	/// `prefix`, ascending.
	pub fn prefix_scan(
		&self,
		domain: Domain,
		prefix: &[u8],
	) -> Result<Vec<Entry<B::Value>>> {
		self.backend.prefix_scan(domain, prefix)
	}

	/// Commit a multi-operation batch atomically.
	pub fn commit(&self, batch: WriteBatch) -> Result<()> { self.backend.commit(batch) }

	/// Put a single `key` → `val` in `domain` as a one-operation atomic commit.
	pub fn put<K, V>(&self, domain: Domain, key: K, val: V) -> Result<()>
	where
		K: Into<Box<[u8]>>,
		V: Into<Box<[u8]>>,
	{
		let mut batch = WriteBatch::new();
		batch.put(domain, key, val);
		self.commit(batch)
	}

	/// Delete a single `key` from `domain` as a one-operation atomic commit.
	pub fn delete<K>(&self, domain: Domain, key: K) -> Result<()>
	where
		K: Into<Box<[u8]>>,
	{
		let mut batch = WriteBatch::new();
		batch.delete(domain, key);
		self.commit(batch)
	}
}
