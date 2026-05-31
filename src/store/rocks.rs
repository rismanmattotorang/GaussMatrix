//! A durable, tuned single-node RocksDB backend.
//!
//! [`RocksBackend`] is the production single-node implementation of
//! [`KvBackend`]. It opens one RocksDB column family per [`Domain`] and maps the
//! abstraction's operations onto RocksDB's column-family API, committing each
//! [`WriteBatch`] through a native `WriteBatch` so the commit is atomic and
//! crash-consistent.
//!
//! This backend is feature-gated behind `rocksdb` because it links the native
//! RocksDB toolchain. Compression defaults to Zstd (the workspace build does not
//! compile in Snappy, RocksDB's default).

use std::{path::Path, sync::Arc};

use rust_rocksdb::{
	BoundColumnFamily, ColumnFamilyDescriptor, DBCompressionType, DBWithThreadMode, Direction,
	IteratorMode, MultiThreaded, Options, WriteBatch as RocksWriteBatch,
};

use crate::{Domain, Entry, KvBackend, Op, Result, StoreError, WriteBatch};

/// The thread-safe RocksDB handle type used across the backend.
type Db = DBWithThreadMode<MultiThreaded>;

/// Durable single-node [`KvBackend`] over RocksDB.
pub struct RocksBackend {
	db: Db,
}

impl RocksBackend {
	/// Open (creating if absent) a RocksDB database at `path` with one column
	/// family per [`Domain`].
	pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
		let mut db_opts = Options::default();
		db_opts.create_if_missing(true);
		db_opts.create_missing_column_families(true);

		let cfds = Domain::ALL
			.into_iter()
			.map(|domain| ColumnFamilyDescriptor::new(domain.name(), cf_options()));

		let db = Db::open_cf_descriptors(&db_opts, path, cfds)?;

		Ok(Self { db })
	}

	/// Resolve the bound column-family handle for `domain`.
	fn cf(&self, domain: Domain) -> Result<Arc<BoundColumnFamily<'_>>> {
		self.db
			.cf_handle(domain.name())
			.ok_or(StoreError::UnknownDomain(domain))
	}
}

/// Per-column-family tuning. Compression is set explicitly to Zstd because the
/// workspace RocksDB build does not include Snappy (RocksDB's default).
fn cf_options() -> Options {
	let mut opts = Options::default();
	opts.set_compression_type(DBCompressionType::Zstd);
	opts
}

impl KvBackend for RocksBackend {
	type Value = Vec<u8>;

	fn get(&self, domain: Domain, key: &[u8]) -> Result<Option<Self::Value>> {
		let cf = self.cf(domain)?;
		let got = self.db.get_pinned_cf(&cf, key)?;
		Ok(got.map(|slice| slice.as_ref().to_vec()))
	}

	fn prefix_scan(
		&self,
		domain: Domain,
		prefix: &[u8],
	) -> Result<Vec<Entry<Self::Value>>> {
		let cf = self.cf(domain)?;
		let mode = IteratorMode::From(prefix, Direction::Forward);

		let mut out = Vec::new();
		for item in self.db.iterator_cf(&cf, mode) {
			let (key, val) = item?;
			if !key.starts_with(prefix) {
				break;
			}
			out.push((key, val.into_vec()));
		}

		Ok(out)
	}

	fn commit(&self, batch: WriteBatch) -> Result<()> {
		let mut wb = RocksWriteBatch::default();
		for op in batch.ops() {
			match op {
				| Op::Put { domain, key, val } => {
					let cf = self.cf(*domain)?;
					wb.put_cf(&cf, key, val);
				},
				| Op::Delete { domain, key } => {
					let cf = self.cf(*domain)?;
					wb.delete_cf(&cf, key);
				},
			}
		}

		self.db.write(&wb)?;
		Ok(())
	}
}
