//! Constructs the [`gm_store`] handle the service core holds.
//!
//! This is the seam between the service core and the Phase-1 pluggable storage
//! abstraction. The single-node profile opens a tuned RocksDB engine at
//! `<database_path>/gm-store` — a directory beside, and independent of, the
//! legacy database, so on-disk migration compatibility is preserved. The
//! returned [`DynStore`] is backend-agnostic: a future deployment can swap in
//! the Phase-2 distributed backend here without changing any consumer.

use std::sync::Arc;

use gaussmatrix_core::{Result, Server, err};
use gm_store::{DynStore, RocksBackend};

/// The subdirectory, under the configured database path, that holds the
/// gm-store-managed column families.
const GM_STORE_DIR: &str = "gm-store";

/// Open the gm-store backing store for the single-node profile.
pub(super) fn open(server: &Arc<Server>) -> Result<DynStore> {
	let path = server.config.database_path.join(GM_STORE_DIR);
	let backend = RocksBackend::open(&path)
		.map_err(|e| err!(Database("failed to open gm-store at {path:?}: {e}")))?;

	Ok(DynStore::new(backend))
}
