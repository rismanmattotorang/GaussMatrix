//! Content-addressed media store (SPECS §III-C).
//!
//! A blob is named by the SHA-256 of its bytes, so identical content is stored
//! exactly once — uploads of the same file deduplicate, and a content id is a
//! self-verifying integrity check. The content id is the base64 of the hash.
//!
//! Two interchangeable backends, chosen by config:
//!
//! * **Local** (default): blobs live in the [`Domain::MediaBlobs`] gm-store
//!   column family — node-local, single-node profile.
//! * **Shared** (`media_cas_provider`): blobs live in a shared object-store
//!   [`Provider`] (e.g. an S3 bucket) under the `media_cas/` prefix, keyed by
//!   content hash. Multiple nodes pointing at the same backend share one
//!   deduplicated namespace; content-addressing makes every write idempotent,
//!   so there are no cross-node write conflicts.
//!
//! This is an additive, self-contained store. The production media path is
//! migrated onto it incrementally, the same staging the other `gm-*` consumers
//! use.

use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gaussmatrix_core::{Result, debug, err, implement, utils::hash::sha256};
use gm_store::{Domain, DynStore};

use crate::storage::Provider;

pub struct Service {
	store: DynStore,
	services: Arc<crate::services::OnceServices>,
}

impl crate::Service for Service {
	fn build(args: &crate::Args<'_>) -> Result<Arc<Self>> {
		Ok(Arc::new(Self { store: args.store.clone(), services: args.services.clone() }))
	}

	fn name(&self) -> &str { crate::service::make_name(std::module_path!()) }
}

/// The shared object-store provider backing the CAS, if `media_cas_provider`
/// names one; otherwise `None` (the local gm-store backend is used).
#[implement(Service)]
fn shared_provider(&self) -> Result<Option<&Arc<Provider>>> {
	match self.services.config.media_cas_provider.as_deref() {
		| None | Some("") => Ok(None),
		| Some(name) => self.services.storage.provider(name).map(Some),
	}
}

/// The content id of `bytes`: the base64 (URL-safe, no pad) of their SHA-256.
/// Pure — the same bytes always yield the same id.
#[implement(Service)]
#[must_use]
pub fn content_id(&self, bytes: &[u8]) -> String { content_id(bytes) }

/// Store `bytes` content-addressed, returning the content id. Identical content
/// is stored exactly once (a re-store is a no-op write of the same key).
#[implement(Service)]
pub async fn store_blob(&self, bytes: &[u8]) -> Result<String> {
	if let Some(provider) = self.shared_provider()? {
		let id = content_id(bytes);
		let path = blob_path(&id);
		debug!(content_id = %id, path = %path, provider = %provider.name, "Storing CAS blob (shared)");
		provider
			.put_one(path.as_str(), bytes.to_vec())
			.await
			.map_err(|e| err!(Database("media blob write failed: {e}")))?;

		return Ok(id);
	}

	store_blob(&self.store, bytes)
}

/// Load the blob for `content_id`, if present.
#[implement(Service)]
pub async fn load_blob(&self, content_id: &str) -> Result<Option<Vec<u8>>> {
	if let Some(provider) = self.shared_provider()? {
		// Validate the id format before touching the backend.
		decode_id(content_id)?;
		let path = blob_path(content_id);
		// A read miss (or unreachable object) is reported as absent, mirroring
		// the local backend's `Option` semantics.
		return Ok(provider.get(path.as_str()).await.ok().map(|bytes| bytes.to_vec()));
	}

	load_blob(&self.store, content_id)
}

/// Whether a blob with `content_id` is stored.
#[implement(Service)]
pub async fn has_blob(&self, content_id: &str) -> Result<bool> {
	if let Some(provider) = self.shared_provider()? {
		decode_id(content_id)?;
		let path = blob_path(content_id);
		return Ok(provider.get(path.as_str()).await.is_ok());
	}

	has_blob(&self.store, content_id)
}

/// The number of distinct content-addressed blobs in the local gm-store
/// backend. (The shared object-store backend is not scanned here.)
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

/// The object-store path of a blob in the shared backend: `media_cas/<ab>/<id>`,
/// sharded by the first two id characters to keep directories shallow. The id is
/// already URL-safe base64, so it is a valid path segment as-is.
fn blob_path(content_id: &str) -> String {
	let shard = content_id.get(..2).unwrap_or("__");
	format!("media_cas/{shard}/{content_id}")
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

	use super::{blob_path, content_id, has_blob, load_blob, store_blob};

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

	#[test]
	fn blob_path_is_sharded_and_stable() {
		let id = content_id(b"payload");
		let path = blob_path(&id);

		// Prefixed namespace, sharded by the first two id characters.
		let shard = id.get(..2).unwrap();
		assert_eq!(path, format!("media_cas/{shard}/{id}"));
		// Stable in the content, not the call.
		assert_eq!(blob_path(&content_id(b"payload")), path);
	}

	/// Integration test of the **shared multi-node backend** scheme against a
	/// real object store (`object_store::InMemory`, the same `ObjectStore` trait
	/// the production `Provider` wraps). Proves the content-addressed layout
	/// dedupes identical uploads to a single object and round-trips bytes — the
	/// property that lets multiple nodes share one backend without conflicts.
	#[tokio::test]
	async fn shared_backend_dedupes_and_round_trips() {
		use futures::StreamExt;
		use object_store::{
			ObjectStore, ObjectStoreExt, PutPayload, memory::InMemory, path::Path,
		};

		let store = InMemory::new();
		let put = async |bytes: &[u8]| -> String {
			let id = content_id(bytes);
			let path = Path::from(blob_path(&id));
			store
				.put(&path, PutPayload::from(bytes.to_vec()))
				.await
				.unwrap();
			id
		};

		// Identical content -> same id/path -> a single stored object (dedup).
		let id = put(b"hello world").await;
		let again = put(b"hello world").await;
		assert_eq!(id, again);

		// Different content -> different id/path -> its own object.
		let other = put(b"goodbye").await;
		assert_ne!(other, id);

		let count = store.list(None).count().await;
		assert_eq!(count, 2, "identical uploads dedupe to one object");

		// Round-trip: the blob reads back byte-for-byte at its content path.
		let path = Path::from(blob_path(&id));
		let got = store.get(&path).await.unwrap().bytes().await.unwrap();
		assert_eq!(got.as_ref(), b"hello world");
	}
}
