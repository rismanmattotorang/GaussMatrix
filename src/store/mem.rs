//! An in-memory reference backend.
//!
//! [`MemBackend`] is a complete, dependency-free implementation of
//! [`KvBackend`] backed by one `BTreeMap` per [`Domain`] under a single
//! `RwLock`. It exists to exercise the abstraction in tests and tooling and to
//! serve as the executable specification a production backend must match. It is
//! not intended for durable deployments.

use std::{
	collections::BTreeMap,
	sync::{Arc, RwLock},
};

use crate::{Domain, Entry, KvBackend, Op, Result, StoreError, WriteBatch};

type Tree = BTreeMap<Box<[u8]>, Arc<[u8]>>;

/// In-memory [`KvBackend`] reference implementation.
///
/// Cloning shares the underlying store (it is an `Arc` internally), matching how
/// a real backend handle is shared across the server.
#[derive(Clone, Default)]
pub struct MemBackend {
	trees: Arc<RwLock<[Tree; Domain::ALL.len()]>>,
}

impl MemBackend {
	/// Create an empty in-memory backend.
	#[must_use]
	pub fn new() -> Self { Self::default() }

	fn poisoned() -> StoreError {
		StoreError::Unavailable("in-memory store lock poisoned".to_owned())
	}
}

impl KvBackend for MemBackend {
	type Value = Arc<[u8]>;

	fn get(&self, domain: Domain, key: &[u8]) -> Result<Option<Self::Value>> {
		let trees = self.trees.read().map_err(|_| Self::poisoned())?;
		Ok(trees[domain.index()].get(key).cloned())
	}

	fn prefix_scan(
		&self,
		domain: Domain,
		prefix: &[u8],
	) -> Result<Vec<Entry<Self::Value>>> {
		let trees = self.trees.read().map_err(|_| Self::poisoned())?;
		let hits = trees[domain.index()]
			.range(prefix.to_vec().into_boxed_slice()..)
			.take_while(|(key, _)| key.starts_with(prefix))
			.map(|(key, val)| (key.clone(), val.clone()))
			.collect();

		Ok(hits)
	}

	fn commit(&self, batch: WriteBatch) -> Result<()> {
		// Holding the single write lock for the whole batch makes the commit
		// atomic: no reader can observe a partial batch.
		let mut trees = self.trees.write().map_err(|_| Self::poisoned())?;
		for op in batch.ops() {
			match op {
				| Op::Put { domain, key, val } => {
					trees[domain.index()].insert(key.clone(), Arc::from(val.as_ref()));
				},
				| Op::Delete { domain, key } => {
					trees[domain.index()].remove(key);
				},
			}
		}

		Ok(())
	}
}
