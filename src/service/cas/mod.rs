//! Content-addressed media store (SPECS §III-C).
//!
//! A blob is named by the SHA-256 of its bytes, so identical content is stored
//! exactly once — uploads of the same file deduplicate, and a content id is a
//! self-verifying integrity check. Blobs live in the [`Domain::MediaBlobs`]
//! gm-store column family (single-node profile); the content id is the base64
//! of the hash.
//!
//! This is an additive, self-contained store. The production media path is
//! migrated onto it incrementally, the same staging the other `gm-*` consumers
//! use.

use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gaussmatrix_core::{Result, err, implement, utils::hash::sha256};
use gm_store::{Domain, DynStore};

pub struct Service {
	store: DynStore,
}

impl crate::Service for Service {
	fn build(args: &crate::Args<'_>) -> Result<Arc<Self>> {
		Ok(Arc::new(Self { store: args.store.clone() }))
	}

	fn name(&self) -> &str { crate::service::make_name(std::module_path!()) }
}

/// The content id of `bytes`: the base64 (URL-safe, no pad) of their SHA-256.
/// Pure — the same bytes always yield the same id.
#[implement(Service)]
#[must_use]
pub fn content_id(&self, bytes: &[u8]) -> String { content_id(bytes) }

/// Store `bytes` content-addressed, returning the content id. Identical content
/// is stored exactly once (a re-store is a no-op write of the same key).
#[implement(Service)]
pub fn store_blob(&self, bytes: &[u8]) -> Result<String> { store_blob(&self.store, bytes) }

/// Load the blob for `content_id`, if present.
#[implement(Service)]
pub fn load_blob(&self, content_id: &str) -> Result<Option<Vec<u8>>> {
	load_blob(&self.store, content_id)
}

/// Whether a blob with `content_id` is stored.
#[implement(Service)]
pub fn has_blob(&self, content_id: &str) -> Result<bool> { has_blob(&self.store, content_id) }

/// The number of distinct content-addressed blobs stored.
#[implement(Service)]
pub fn blob_count(&self) -> Result<usize> {
	self.store
		.prefix_scan(Domain::MediaBlobs, b"")
		.map(|scanned| scanned.len())
		.map_err(|e| err!(Database("media blob scan failed: {e}")))
}

/// The content id of `bytes`: base64(URL-safe, no pad) of `SHA-256(bytes)`.
fn content_id(bytes: &[u8]) -> String { URL_SAFE_NO_PAD.encode(sha256::hash(bytes)) }

/// Decode a content id back to the raw hash used as the store key.
fn decode_id(content_id: &str) -> Result<Vec<u8>> {
	URL_SAFE_NO_PAD
		.decode(content_id)
		.map_err(|_| err!(Request(InvalidParam("invalid media content id"))))
}

/// Store `bytes` under their content hash, returning the content id.
fn store_blob(store: &DynStore, bytes: &[u8]) -> Result<String> {
	let id = content_id(bytes);
	let key = sha256::hash(bytes).to_vec();
	store
		.put(Domain::MediaBlobs, key, bytes.to_vec())
		.map_err(|e| err!(Database("media blob write failed: {e}")))?;

	Ok(id)
}

/// Load the blob for `content_id`, if present.
fn load_blob(store: &DynStore, content_id: &str) -> Result<Option<Vec<u8>>> {
	let key = decode_id(content_id)?;
	store
		.get(Domain::MediaBlobs, &key)
		.map_err(|e| err!(Database("media blob read failed: {e}")))
}

/// Whether a blob with `content_id` is stored.
fn has_blob(store: &DynStore, content_id: &str) -> Result<bool> {
	let key = decode_id(content_id)?;
	store
		.contains(Domain::MediaBlobs, &key)
		.map_err(|e| err!(Database("media blob lookup failed: {e}")))
}

#[cfg(test)]
mod tests {
	use gm_store::{DynStore, MemBackend};

	use super::{content_id, has_blob, load_blob, store_blob};

	fn mem_store() -> DynStore { DynStore::new(MemBackend::default()) }

	#[test]
	fn store_is_content_addressed_and_dedupes() {
		let store = mem_store();

		let id = store_blob(&store, b"hello world").unwrap();
		// The id is stable in the content, not the call.
		assert_eq!(id, content_id(b"hello world"));

		// Re-storing identical content yields the same id and no second blob.
		let again = store_blob(&store, b"hello world").unwrap();
		assert_eq!(again, id);
		assert_eq!(store.prefix_scan(super::Domain::MediaBlobs, b"").unwrap().len(), 1);

		// Different content gets a different id and its own blob.
		let other = store_blob(&store, b"goodbye").unwrap();
		assert_ne!(other, id);
		assert_eq!(store.prefix_scan(super::Domain::MediaBlobs, b"").unwrap().len(), 2);
	}

	#[test]
	fn load_and_has_round_trip() {
		let store = mem_store();
		let id = store_blob(&store, b"payload").unwrap();

		assert!(has_blob(&store, &id).unwrap());
		assert_eq!(load_blob(&store, &id).unwrap(), Some(b"payload".to_vec()));

		// An unknown (but well-formed) id is simply absent.
		let absent = content_id(b"never stored");
		assert!(!has_blob(&store, &absent).unwrap());
		assert_eq!(load_blob(&store, &absent).unwrap(), None);

		// A malformed id is rejected.
		load_blob(&store, "not valid base64!!").unwrap_err();
	}
}
