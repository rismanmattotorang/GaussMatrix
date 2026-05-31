//! Minimal append-only audit log backed by gm-store.
//!
//! This is the first consumer wired onto the Phase-1 pluggable storage
//! abstraction (`Services.store`). It owns the [`Domain::AuditLog`] column
//! family and records ordered, append-only entries — the storage foundation for
//! the tamper-evident, hash-chained agent audit log of the specification
//! (§IV-D). (The cryptographic hash chain itself is a Phase-3 refinement; this
//! increment establishes the durable, ordered append/read path.)
//!
//! Because the audit log has no data in the inherited database, backing it with
//! gm-store is a clean, non-breaking first use of the storage seam.

use std::sync::{
	Arc,
	atomic::{AtomicU64, Ordering},
};

use gaussmatrix_core::{Result, err, implement};
use gm_store::{Domain, DynStore};

/// One audit record: its sequence number and opaque payload.
pub type AuditEntry = (u64, Vec<u8>);

pub struct Service {
	store: DynStore,
	/// The next sequence number to assign; initialised from the store at build
	/// and advanced atomically per append.
	next_seq: AtomicU64,
}

impl crate::Service for Service {
	fn build(args: &crate::Args<'_>) -> Result<Arc<Self>> {
		let store = args.store.clone();
		let next_seq = AtomicU64::new(next_sequence(&store)?);

		Ok(Arc::new(Self { store, next_seq }))
	}

	fn name(&self) -> &str { crate::service::make_name(std::module_path!()) }
}

/// Append `payload` to the audit log, returning its assigned sequence number.
#[implement(Service)]
pub fn append(&self, payload: &[u8]) -> Result<u64> {
	let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
	self.store
		.put(Domain::AuditLog, seq.to_be_bytes().to_vec(), payload.to_vec())
		.map_err(|e| err!(Database("audit log append failed: {e}")))?;

	Ok(seq)
}

/// All audit entries, ascending by sequence number.
#[implement(Service)]
pub fn entries(&self) -> Result<Vec<AuditEntry>> { read_entries(&self.store) }

/// The number of entries currently recorded.
#[implement(Service)]
pub fn count(&self) -> Result<usize> { Ok(read_entries(&self.store)?.len()) }

/// Decode an 8-byte big-endian sequence key.
fn decode_seq(key: &[u8]) -> Result<u64> {
	let bytes = <[u8; 8]>::try_from(key)
		.map_err(|_| err!(Database("audit log key is not an 8-byte sequence")))?;

	Ok(u64::from_be_bytes(bytes))
}

/// Read all entries from `store`, ascending by sequence number.
fn read_entries(store: &DynStore) -> Result<Vec<AuditEntry>> {
	let scanned = store
		.prefix_scan(Domain::AuditLog, b"")
		.map_err(|e| err!(Database("audit log scan failed: {e}")))?;

	let mut out = Vec::with_capacity(scanned.len());
	for (key, val) in scanned {
		out.push((decode_seq(&key)?, val));
	}

	Ok(out)
}

/// The next sequence number for `store`: one past the highest stored, or zero
/// when empty.
fn next_sequence(store: &DynStore) -> Result<u64> {
	let next = read_entries(store)?
		.last()
		.map_or(0, |(seq, _)| seq.saturating_add(1));

	Ok(next)
}

#[cfg(test)]
mod tests {
	use std::sync::atomic::AtomicU64;

	use gm_store::{DynStore, MemBackend};

	use super::{Service, next_sequence};

	fn service() -> Service {
		Service {
			store: DynStore::new(MemBackend::new()),
			next_seq: AtomicU64::new(0),
		}
	}

	#[test]
	fn append_assigns_sequential_ids_and_reads_back_in_order() {
		let svc = service();
		assert_eq!(svc.append(b"a").unwrap(), 0);
		assert_eq!(svc.append(b"b").unwrap(), 1);
		assert_eq!(svc.append(b"c").unwrap(), 2);

		let payloads: Vec<_> =
			svc.entries().unwrap().into_iter().map(|(_, val)| val).collect();
		assert_eq!(payloads, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
		assert_eq!(svc.count().unwrap(), 3);
	}

	#[test]
	fn empty_log_has_no_entries() {
		let svc = service();
		assert_eq!(svc.count().unwrap(), 0);
		assert!(svc.entries().unwrap().is_empty());
	}

	#[test]
	fn sequence_resumes_after_existing_entries() {
		let store = DynStore::new(MemBackend::new());
		let svc = Service { store: store.clone(), next_seq: AtomicU64::new(0) };
		svc.append(b"x").unwrap();
		svc.append(b"y").unwrap();

		// A fresh service over the same store must resume at the next sequence.
		assert_eq!(next_sequence(&store).unwrap(), 2);
	}
}
