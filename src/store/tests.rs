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
