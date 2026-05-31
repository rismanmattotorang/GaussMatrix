//! A type-erased store handle.
//!
//! [`KvBackend`] has an associated `Value` type, which makes it not
//! object-safe: a consumer cannot hold a `Box<dyn KvBackend>`. The service core,
//! however, wants to hold *one* store field whose concrete backend (RocksDB,
//! in-memory, or a future distributed engine) is chosen at construction time.
//!
//! [`DynStore`] provides that: an object-safe [`DynBackend`] view whose value
//! handle is erased to an owned `Vec<u8>`, plus a cloneable handle with the same
//! ergonomic surface as [`Store`](crate::Store). Every [`KvBackend`] is a
//! [`DynBackend`] automatically.

use std::sync::Arc;

use crate::{Domain, Entry, KvBackend, Result, WriteBatch};

/// An object-safe view of a backend, with the value handle erased to `Vec<u8>`.
///
/// This is implemented for every [`KvBackend`] by the blanket impl below, so it
/// is rarely implemented by hand; it exists to be used as `dyn DynBackend`.
pub trait DynBackend: Send + Sync + 'static {
	/// See [`KvBackend::get`].
	fn get(&self, domain: Domain, key: &[u8]) -> Result<Option<Vec<u8>>>;

	/// See [`KvBackend::contains`].
	fn contains(&self, domain: Domain, key: &[u8]) -> Result<bool>;

	/// See [`KvBackend::prefix_scan`].
	fn prefix_scan(&self, domain: Domain, prefix: &[u8]) -> Result<Vec<Entry<Vec<u8>>>>;

	/// See [`KvBackend::commit`].
	fn commit(&self, batch: WriteBatch) -> Result<()>;
}

impl<B: KvBackend> DynBackend for B {
	fn get(&self, domain: Domain, key: &[u8]) -> Result<Option<Vec<u8>>> {
		Ok(KvBackend::get(self, domain, key)?.map(|val| val.as_ref().to_vec()))
	}

	fn contains(&self, domain: Domain, key: &[u8]) -> Result<bool> {
		KvBackend::contains(self, domain, key)
	}

	fn prefix_scan(&self, domain: Domain, prefix: &[u8]) -> Result<Vec<Entry<Vec<u8>>>> {
		Ok(KvBackend::prefix_scan(self, domain, prefix)?
			.into_iter()
			.map(|(key, val)| (key, val.as_ref().to_vec()))
			.collect())
	}

	fn commit(&self, batch: WriteBatch) -> Result<()> { KvBackend::commit(self, batch) }
}

/// A cloneable, backend-agnostic store handle.
///
/// `DynStore` is the type the service core holds: it owns an
/// `Arc<dyn DynBackend>`, so the same field can carry a RocksDB-backed store in
/// production and an in-memory one in tests. Cloning shares the backend.
#[derive(Clone)]
pub struct DynStore {
	backend: Arc<dyn DynBackend>,
}

impl DynStore {
	/// Erase a concrete backend into a `DynStore`.
	pub fn new<B: KvBackend>(backend: B) -> Self { Self { backend: Arc::new(backend) } }

	/// Wrap an already-erased backend handle.
	#[must_use]
	pub fn from_dyn(backend: Arc<dyn DynBackend>) -> Self { Self { backend } }

	/// Fetch the value for `key` in `domain`, if present.
	pub fn get(&self, domain: Domain, key: &[u8]) -> Result<Option<Vec<u8>>> {
		self.backend.get(domain, key)
	}

	/// Whether `domain` contains `key`.
	pub fn contains(&self, domain: Domain, key: &[u8]) -> Result<bool> {
		self.backend.contains(domain, key)
	}

	/// Snapshot every `(key, value)` in `domain` whose key starts with `prefix`.
	pub fn prefix_scan(
		&self,
		domain: Domain,
		prefix: &[u8],
	) -> Result<Vec<Entry<Vec<u8>>>> {
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
