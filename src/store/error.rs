//! Error and result types for the storage abstraction.

use std::{error::Error, fmt};

/// Result alias for fallible storage operations.
pub type Result<T, E = StoreError> = std::result::Result<T, E>;

/// Errors that a [`KvBackend`](crate::KvBackend) may surface.
///
/// The variants are intentionally backend-neutral: a RocksDB, in-memory, or
/// distributed backend maps its native failures onto this enum so the service
/// core never depends on a concrete engine's error type.
#[derive(Debug)]
#[non_exhaustive]
pub enum StoreError {
	/// A referenced column family / domain is not open in this backend.
	UnknownDomain(Domain),

	/// The backend rejected an otherwise well-formed operation (e.g. a failed
	/// atomic commit). Carries a human-readable, backend-supplied message.
	Backend(String),

	/// The backend is unavailable — not yet opened, shutting down, or (for a
	/// distributed backend) partitioned away from the owning shard.
	Unavailable(String),

	/// A stored value could not be interpreted in the expected form.
	Corrupt(String),
}

use crate::Domain;

impl fmt::Display for StoreError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			| Self::UnknownDomain(domain) => {
				write!(f, "unknown storage domain: {}", domain.name())
			},
			| Self::Backend(msg) => write!(f, "storage backend error: {msg}"),
			| Self::Unavailable(msg) => write!(f, "storage backend unavailable: {msg}"),
			| Self::Corrupt(msg) => write!(f, "corrupt stored value: {msg}"),
		}
	}
}

impl Error for StoreError {}
