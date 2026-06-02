//! Tamper-evident, hash-chained audit log backed by gm-store.
//!
//! This is the first consumer wired onto the Phase-1 pluggable storage
//! abstraction (`Services.store`). It owns the [`Domain::AuditLog`] column
//! family and records ordered, append-only entries, each committing to the hash
//! of its predecessor so any retroactive edit is detectable — the
//! compliance backbone of the specification (§IV-D).
//!
//! Each entry is stored as `prev_hash (32 bytes) || payload`, and its own hash
//! is `SHA-256(seq_be || prev_hash || payload)`, which becomes the next entry's
//! `prev_hash`. [`Service::verify`] walks the chain checking sequence
//! contiguity and link integrity, and confirms the persisted chain still hashes
//! to the in-process head — catching both a broken mid-chain link and a rewrite
//! of the final entry.
//!
//! Because the audit log has no data in the inherited database, backing it with
//! gm-store is a clean, non-breaking use of the storage seam.

use std::sync::{Arc, Mutex};

use gaussmatrix_core::{Result, err, implement, utils::hash::sha256};
use gm_store::{Domain, DynStore};

/// Length of a SHA-256 digest, the per-entry chain link.
const HASH_LEN: usize = 32;

/// A chain link / entry hash.
type Hash = [u8; HASH_LEN];

/// The genesis predecessor hash for the first entry.
const GENESIS: Hash = [0_u8; HASH_LEN];

/// One audit record: its sequence number and opaque payload.
pub type AuditEntry = (u64, Vec<u8>);

/// The mutable tail of the chain, serialising appends.
struct ChainHead {
	/// The next sequence number to assign.
	next_seq: u64,
	/// The hash of the last appended entry (or [`GENESIS`] when empty).
	hash: Hash,
}

pub struct Service {
	store: DynStore,
	head: Mutex<ChainHead>,
}

impl crate::Service for Service {
	fn build(args: &crate::Args<'_>) -> Result<Arc<Self>> {
		let store = args.store.clone();
		let head = recover_head(&store)?;

		Ok(Arc::new(Self { store, head: Mutex::new(head) }))
	}

	fn name(&self) -> &str { crate::service::make_name(std::module_path!()) }
}

/// Append `payload` to the audit log, returning its assigned sequence number.
///
/// The entry commits to the current chain head, and the head advances to this
/// entry's hash. Appends are serialised so the chain stays well-formed under
/// concurrency.
#[implement(Service)]
pub fn append(&self, payload: &[u8]) -> Result<u64> {
	let mut head = self
		.head
		.lock()
		.map_err(|_| err!(Database("audit log lock poisoned")))?;

	let seq = head.next_seq;
	let this_hash = entry_hash(seq, &head.hash, payload);

	let mut value = Vec::with_capacity(HASH_LEN.saturating_add(payload.len()));
	value.extend_from_slice(&head.hash);
	value.extend_from_slice(payload);

	self.store
		.put(Domain::AuditLog, seq.to_be_bytes().to_vec(), value)
		.map_err(|e| err!(Database("audit log append failed: {e}")))?;

	head.next_seq = seq.saturating_add(1);
	head.hash = this_hash;

	Ok(seq)
}

/// All audit entries, ascending by sequence number.
#[implement(Service)]
pub fn entries(&self) -> Result<Vec<AuditEntry>> { read_entries(&self.store) }

/// The most recent `n` audit entries, ascending by sequence number.
#[implement(Service)]
pub fn tail(&self, n: usize) -> Result<Vec<AuditEntry>> {
	let mut all = read_entries(&self.store)?;
	let start = all.len().saturating_sub(n);
	Ok(all.split_off(start))
}

/// Export the whole audit log as JSON Lines — one `{"seq":N,"record":…}` object
/// per entry — for compliance review and offline verification. A payload that
/// is not valid JSON is exported as a string.
#[implement(Service)]
pub fn export_jsonl(&self) -> Result<String> {
	use std::fmt::Write as _;

	let mut out = String::new();
	for (seq, payload) in read_entries(&self.store)? {
		let record: serde_json::Value = serde_json::from_slice(&payload).unwrap_or_else(|_| {
			serde_json::Value::String(String::from_utf8_lossy(&payload).into_owned())
		});
		let line = serde_json::json!({ "seq": seq, "record": record });
		let _ = writeln!(out, "{line}");
	}

	Ok(out)
}

/// The number of entries currently recorded.
#[implement(Service)]
pub fn count(&self) -> Result<usize> { Ok(read_entries(&self.store)?.len()) }

/// The current chain-head hash — `SHA-256` over the whole appended chain (or the
/// genesis value when empty). Committing to this value attests to the entire
/// log's contents.
#[implement(Service)]
pub fn head_hash(&self) -> Result<Vec<u8>> {
	let head = self
		.head
		.lock()
		.map_err(|_| err!(Database("audit log lock poisoned")))?;

	Ok(head.hash.to_vec())
}

/// Verify the integrity of the hash chain.
///
/// Returns an error identifying the first inconsistency: a sequence gap, a
/// too-short entry, a broken link (an entry whose stored predecessor hash does
/// not match the recomputed hash of the previous entry), or a head mismatch (the
/// persisted chain no longer hashes to the in-process head — i.e. the final
/// entry was rewritten).
#[implement(Service)]
pub fn verify(&self) -> Result<()> {
	let mut expected_prev = GENESIS;
	let mut expected_seq: u64 = 0;

	for (seq, prev, payload) in read_raw(&self.store)? {
		if seq != expected_seq {
			return Err(err!(Database("audit log sequence gap at {seq}")));
		}
		if prev != expected_prev {
			return Err(err!(Database("audit log broken link at seq {seq}")));
		}

		expected_prev = entry_hash(seq, &prev, &payload);
		expected_seq = seq.saturating_add(1);
	}

	let head = self
		.head
		.lock()
		.map_err(|_| err!(Database("audit log lock poisoned")))?;
	if expected_prev != head.hash || expected_seq != head.next_seq {
		return Err(err!(Database("audit log head mismatch — tampering detected")));
	}

	Ok(())
}

/// The hash of an entry: `SHA-256(seq_be || prev_hash || payload)`.
fn entry_hash(seq: u64, prev: &[u8], payload: &[u8]) -> Hash {
	let seq_be = seq.to_be_bytes();
	sha256::concat([seq_be.as_slice(), prev, payload].into_iter())
}

/// Decode an 8-byte big-endian sequence key.
fn decode_seq(key: &[u8]) -> Result<u64> {
	let bytes = <[u8; 8]>::try_from(key)
		.map_err(|_| err!(Database("audit log key is not an 8-byte sequence")))?;

	Ok(u64::from_be_bytes(bytes))
}

/// Read every stored entry as `(seq, prev_hash, payload)`, ascending.
fn read_raw(store: &DynStore) -> Result<Vec<(u64, Hash, Vec<u8>)>> {
	let scanned = store
		.prefix_scan(Domain::AuditLog, b"")
		.map_err(|e| err!(Database("audit log scan failed: {e}")))?;

	let mut out = Vec::with_capacity(scanned.len());
	for (key, value) in scanned {
		let seq = decode_seq(&key)?;
		if value.len() < HASH_LEN {
			return Err(err!(Database("audit log entry {seq} is too short")));
		}
		let (prev, payload) = value.split_at(HASH_LEN);
		let prev: Hash = prev
			.try_into()
			.map_err(|_| err!(Database("audit log entry {seq} has a malformed link")))?;
		out.push((seq, prev, payload.to_vec()));
	}

	Ok(out)
}

/// Read all entries as `(seq, payload)`, stripping the chain link.
fn read_entries(store: &DynStore) -> Result<Vec<AuditEntry>> {
	Ok(read_raw(store)?
		.into_iter()
		.map(|(seq, _prev, payload)| (seq, payload))
		.collect())
}

/// Recover the chain head from the store at startup.
fn recover_head(store: &DynStore) -> Result<ChainHead> {
	let mut head = ChainHead { next_seq: 0, hash: GENESIS };
	for (seq, prev, payload) in read_raw(store)? {
		head.hash = entry_hash(seq, &prev, &payload);
		head.next_seq = seq.saturating_add(1);
	}

	Ok(head)
}

#[cfg(test)]
mod tests {
	use gm_store::{Domain, DynStore, MemBackend};

	use super::{ChainHead, GENESIS, Service, recover_head};

	fn service() -> (Service, DynStore) {
		let store = DynStore::new(MemBackend::new());
		let service = Service {
			store: store.clone(),
			head: std::sync::Mutex::new(ChainHead { next_seq: 0, hash: GENESIS }),
		};
		(service, store)
	}

	#[test]
	fn append_chains_and_reads_back_in_order() {
		let (svc, _store) = service();
		assert_eq!(svc.append(b"a").unwrap(), 0);
		assert_eq!(svc.append(b"b").unwrap(), 1);
		assert_eq!(svc.append(b"c").unwrap(), 2);

		let payloads: Vec<_> =
			svc.entries().unwrap().into_iter().map(|(_, val)| val).collect();
		assert_eq!(payloads, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
		assert_eq!(svc.count().unwrap(), 3);
	}

	#[test]
	fn intact_chain_verifies() {
		let (svc, _store) = service();
		for entry in [b"one".as_slice(), b"two".as_slice(), b"three".as_slice()] {
			svc.append(entry).unwrap();
		}
		svc.verify().unwrap();
	}

	#[test]
	fn empty_chain_verifies() {
		let (svc, _store) = service();
		svc.verify().unwrap();
	}

	#[test]
	fn tampering_a_middle_entry_breaks_the_link() {
		let (svc, store) = service();
		svc.append(b"first").unwrap();
		svc.append(b"second").unwrap();
		svc.append(b"third").unwrap();

		// Overwrite entry 1's payload directly in the store, preserving its
		// stored predecessor hash — entry 2's link no longer matches.
		let original = store.get(Domain::AuditLog, &1_u64.to_be_bytes()).unwrap().unwrap();
		let mut tampered = original[..super::HASH_LEN].to_vec();
		tampered.extend_from_slice(b"FORGED");
		store.put(Domain::AuditLog, 1_u64.to_be_bytes().to_vec(), tampered).unwrap();

		assert!(svc.verify().is_err(), "tampering must be detected");
	}

	#[test]
	fn tampering_the_last_entry_is_detected_by_head_mismatch() {
		let (svc, store) = service();
		svc.append(b"first").unwrap();
		svc.append(b"last").unwrap();

		let original = store.get(Domain::AuditLog, &1_u64.to_be_bytes()).unwrap().unwrap();
		let mut tampered = original[..super::HASH_LEN].to_vec();
		tampered.extend_from_slice(b"FORGED");
		store.put(Domain::AuditLog, 1_u64.to_be_bytes().to_vec(), tampered).unwrap();

		assert!(svc.verify().is_err(), "final-entry rewrite must be detected");
	}

	#[test]
	fn recover_head_resumes_the_chain() {
		let (svc, store) = service();
		svc.append(b"x").unwrap();
		svc.append(b"y").unwrap();

		// A fresh head recovered from the store resumes at seq 2 and matches the
		// running head, so a service rebuilt over the same store verifies.
		let recovered = recover_head(&store).unwrap();
		assert_eq!(recovered.next_seq, 2);
		assert_eq!(recovered.hash, svc.head.lock().unwrap().hash);
	}
}
