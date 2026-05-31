//! # gm-store — GaussMatrix pluggable storage abstraction
//!
//! `gm-store` is the storage cornerstone of the GaussMatrix server core
//! ([`GaussMatrix-SPECS.pdf`], §III-C, Phase 1). It generalises the tuned
//! RocksDB integration inherited from the Tuwunel/Conduit lineage into a
//! **backend-agnostic key/value trait** with **explicit, per-domain column
//! families**.
//!
//! The abstraction exists so that a deployment can choose its storage posture
//! — a tuned single-node RocksDB engine, or (Phase 2) a horizontally
//! partitioned distributed KV store — *without touching the service core*. The
//! schema lives behind the [`KvBackend`] trait; the rest of the server speaks
//! only in [`Domain`]s, keys, and atomic [`WriteBatch`]es.
//!
//! ## Design invariants (from the specification)
//!
//! * **Per-domain column families.** Every logical store is one of the nine
//!   [`Domain`]s — events, room state, the auth-chain index, the resolved-state
//!   cache, media metadata, the device store, the key store, account data, and
//!   the tamper-evident audit log.
//! * **Atomic, batched writes.** A request's mutations are gathered into a
//!   single [`WriteBatch`] and committed all-or-nothing, so an accepted event
//!   and its state-delta become visible together.
//! * **Cheap value handles.** Reads return the backend's own value handle
//!   ([`KvBackend::Value`]); the in-memory reference backend hands back a shared
//!   `Arc<[u8]>`. (True pinned-slice zero-copy on RocksDB is a tracked
//!   refinement that requires a lifetime-generic value type.)
//! * **Pluggable backends.** [`KvBackend`] is the single seam. This crate ships
//!   the [`MemBackend`] reference implementation and the durable single-node
//!   [`RocksBackend`] (feature `rocksdb`); the Phase-2 distributed backend
//!   implements the same trait.
//!
//! ## Example
//!
//! ```
//! use gm_store::{Domain, MemBackend, Store, WriteBatch};
//!
//! let store = Store::new(MemBackend::new());
//!
//! // Atomically commit an event and its state pointer together.
//! let mut batch = WriteBatch::new();
//! batch.put(Domain::Events, b"$evt:example.org".to_vec(), b"<pdu bytes>".to_vec());
//! batch.put(Domain::RoomState, b"!room:example.org/m.room.name".to_vec(), b"$evt:example.org".to_vec());
//! store.commit(batch).unwrap();
//!
//! let pdu = store.get(Domain::Events, b"$evt:example.org").unwrap();
//! assert_eq!(pdu.as_deref(), Some(&b"<pdu bytes>"[..]));
//! ```
//!
//! [`GaussMatrix-SPECS.pdf`]: ../../GaussMatrix-SPECS.pdf

#![forbid(unsafe_code)]

mod backend;
mod domain;
mod dyn_store;
mod error;
mod mem;
#[cfg(feature = "rocksdb")]
mod rocks;
mod store;
#[cfg(test)]
mod tests;

#[cfg(feature = "rocksdb")]
pub use self::rocks::RocksBackend;
pub use self::{
	backend::{Entry, KvBackend, Op, WriteBatch},
	domain::Domain,
	dyn_store::{DynBackend, DynStore},
	error::{Result, StoreError},
	mem::MemBackend,
	store::Store,
};
