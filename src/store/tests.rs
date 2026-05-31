//! Unit tests for the storage abstraction and the in-memory reference backend.

use crate::{Domain, MemBackend, Store, WriteBatch};

fn store() -> Store<MemBackend> { Store::new(MemBackend::new()) }

#[test]
fn domain_names_are_unique_and_roundtrip() {
	let mut seen = std::collections::BTreeSet::new();
	for domain in Domain::ALL {
		assert!(seen.insert(domain.name()), "duplicate domain name: {}", domain.name());
		assert_eq!(Domain::from_name(domain.name()), Some(domain));
		assert_eq!(Domain::ALL[domain.index()], domain);
	}
	assert_eq!(seen.len(), Domain::ALL.len());
	assert_eq!(Domain::from_name("nonexistent"), None);
}

#[test]
fn put_then_get() {
	let s = store();
	s.put(Domain::Events, b"k".to_vec(), b"v".to_vec()).unwrap();
	let got = s.get(Domain::Events, b"k").unwrap();
	assert_eq!(got.as_deref(), Some(&b"v"[..]));
	assert_eq!(s.get(Domain::Events, b"absent").unwrap().as_deref(), None);
}

#[test]
fn overwrite_replaces_value() {
	let s = store();
	s.put(Domain::AccountData, b"k".to_vec(), b"v1".to_vec()).unwrap();
	s.put(Domain::AccountData, b"k".to_vec(), b"v2".to_vec()).unwrap();
	assert_eq!(s.get(Domain::AccountData, b"k").unwrap().as_deref(), Some(&b"v2"[..]));
}

#[test]
fn delete_removes_key() {
	let s = store();
	s.put(Domain::DeviceStore, b"k".to_vec(), b"v".to_vec()).unwrap();
	assert!(s.contains(Domain::DeviceStore, b"k").unwrap());
	s.delete(Domain::DeviceStore, b"k".to_vec()).unwrap();
	assert!(!s.contains(Domain::DeviceStore, b"k").unwrap());
}

#[test]
fn domains_are_isolated() {
	let s = store();
	s.put(Domain::Events, b"k".to_vec(), b"event".to_vec()).unwrap();
	s.put(Domain::RoomState, b"k".to_vec(), b"state".to_vec()).unwrap();
	assert_eq!(s.get(Domain::Events, b"k").unwrap().as_deref(), Some(&b"event"[..]));
	assert_eq!(s.get(Domain::RoomState, b"k").unwrap().as_deref(), Some(&b"state"[..]));
}

#[test]
fn batch_commits_all_operations_atomically() {
	let s = store();
	s.put(Domain::KeyStore, b"old".to_vec(), b"x".to_vec()).unwrap();

	let mut batch = WriteBatch::new();
	batch
		.put(Domain::Events, b"$e".to_vec(), b"pdu".to_vec())
		.put(Domain::RoomState, b"!r/name".to_vec(), b"$e".to_vec())
		.delete(Domain::KeyStore, b"old".to_vec());
	assert_eq!(batch.len(), 3);
	assert!(!batch.is_empty());

	s.commit(batch).unwrap();

	assert_eq!(s.get(Domain::Events, b"$e").unwrap().as_deref(), Some(&b"pdu"[..]));
	assert_eq!(s.get(Domain::RoomState, b"!r/name").unwrap().as_deref(), Some(&b"$e"[..]));
	assert!(!s.contains(Domain::KeyStore, b"old").unwrap());
}

#[test]
fn prefix_scan_is_ordered_and_bounded() {
	let s = store();
	for key in ["b:1", "a:2", "a:1", "a:10", "c:1"] {
		s.put(Domain::AuditLog, key.as_bytes().to_vec(), b"_".to_vec())
			.unwrap();
	}

	let hits = s.prefix_scan(Domain::AuditLog, b"a:").unwrap();
	let keys: Vec<_> = hits
		.iter()
		.map(|(k, _)| String::from_utf8(k.to_vec()).unwrap())
		.collect();

	// Only "a:" keys, in ascending byte order.
	assert_eq!(keys, vec!["a:1", "a:10", "a:2"]);
}

#[test]
fn empty_prefix_scans_whole_domain() {
	let s = store();
	s.put(Domain::MediaMetadata, b"m1".to_vec(), b"_".to_vec()).unwrap();
	s.put(Domain::MediaMetadata, b"m2".to_vec(), b"_".to_vec()).unwrap();
	assert_eq!(s.prefix_scan(Domain::MediaMetadata, b"").unwrap().len(), 2);
}

#[test]
fn cloned_store_shares_backend() {
	let s = store();
	let s2 = s.clone();
	s.put(Domain::ResolvedStateCache, b"k".to_vec(), b"v".to_vec()).unwrap();
	assert_eq!(s2.get(Domain::ResolvedStateCache, b"k").unwrap().as_deref(), Some(&b"v"[..]));
}

#[test]
fn auth_chain_index_prefix_isolation() {
	let s = store();
	s.put(Domain::AuthChainIndex, b"room1:a".to_vec(), b"_".to_vec()).unwrap();
	s.put(Domain::AuthChainIndex, b"room2:a".to_vec(), b"_".to_vec()).unwrap();
	assert_eq!(s.prefix_scan(Domain::AuthChainIndex, b"room1:").unwrap().len(), 1);
}

#[cfg(feature = "rocksdb")]
mod rocksdb_backend {
	use std::{
		path::PathBuf,
		sync::atomic::{AtomicU64, Ordering},
		time::{SystemTime, UNIX_EPOCH},
	};

	use crate::{Domain, RocksBackend, Store, WriteBatch};

	/// A unique temporary directory removed when dropped — avoids a dev
	/// dependency on `tempfile` for a single test.
	struct TempDir {
		path: PathBuf,
	}

	impl TempDir {
		fn new() -> Self {
			static COUNTER: AtomicU64 = AtomicU64::new(0);
			let nanos = SystemTime::now()
				.duration_since(UNIX_EPOCH)
				.unwrap()
				.as_nanos();
			let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
			let mut path = std::env::temp_dir();
			path.push(format!("gm_store_rocks_{}_{nanos}_{seq}", std::process::id()));
			std::fs::create_dir_all(&path).unwrap();
			Self { path }
		}
	}

	impl Drop for TempDir {
		fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.path); }
	}

	#[test]
	fn open_put_get_batch_scan_delete_roundtrip() {
		let dir = TempDir::new();
		let store = Store::new(RocksBackend::open(&dir.path).unwrap());

		// Single put/get.
		store.put(Domain::Events, b"$e1".to_vec(), b"pdu1".to_vec()).unwrap();
		assert_eq!(store.get(Domain::Events, b"$e1").unwrap().as_deref(), Some(&b"pdu1"[..]));

		// Atomic multi-domain batch.
		let mut batch = WriteBatch::new();
		batch
			.put(Domain::Events, b"$e2".to_vec(), b"pdu2".to_vec())
			.put(Domain::RoomState, b"!r/name".to_vec(), b"$e2".to_vec())
			.delete(Domain::Events, b"$e1".to_vec());
		store.commit(batch).unwrap();

		assert!(!store.contains(Domain::Events, b"$e1").unwrap());
		assert_eq!(store.get(Domain::RoomState, b"!r/name").unwrap().as_deref(), Some(&b"$e2"[..]));

		// Prefix scan stays within the domain and prefix, ascending.
		for key in ["a:1", "a:2", "b:1"] {
			store.put(Domain::AuditLog, key.as_bytes().to_vec(), b"_".to_vec()).unwrap();
		}
		let keys: Vec<_> = store
			.prefix_scan(Domain::AuditLog, b"a:")
			.unwrap()
			.into_iter()
			.map(|(k, _)| String::from_utf8(k.to_vec()).unwrap())
			.collect();
		assert_eq!(keys, vec!["a:1", "a:2"]);
	}

	#[test]
	fn data_persists_across_reopen() {
		let dir = TempDir::new();
		{
			let store = Store::new(RocksBackend::open(&dir.path).unwrap());
			store.put(Domain::AccountData, b"k".to_vec(), b"v".to_vec()).unwrap();
		}
		// Reopen the same path; the value must still be there.
		let store = Store::new(RocksBackend::open(&dir.path).unwrap());
		assert_eq!(store.get(Domain::AccountData, b"k").unwrap().as_deref(), Some(&b"v"[..]));
	}
}
