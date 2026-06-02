//! The per-domain column-family model.
//!
//! The specification (§III-C) requires "explicit, per-domain column families"
//! rather than an undifferentiated key space. Each [`Domain`] is one logical
//! column family with its own stable on-disk name, isolating access patterns so
//! a backend can tune compression, caching, and partitioning per domain.

use std::fmt;

/// A logical storage domain — one column family in backend terms.
///
/// The set is closed and stable: domain names are part of the on-disk contract,
/// so a tuned RocksDB backend and a distributed backend agree on them, and
/// migration tooling can reason about them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Domain {
	/// Persisted Matrix events (PDUs) keyed by event id.
	Events = 0,

	/// Current and historical room state mappings.
	RoomState = 1,

	/// The authorization-chain index used by state resolution.
	AuthChainIndex = 2,

	/// Memoised outputs of the state-resolution algorithm.
	ResolvedStateCache = 3,

	/// Metadata for media blobs (the blobs themselves live in the object store).
	MediaMetadata = 4,

	/// Per-device records (one-time keys, device lists, display data).
	DeviceStore = 5,

	/// Cross-signing, key-backup, and key-claiming material (no plaintext).
	KeyStore = 6,

	/// Per-user and per-room account data.
	AccountData = 7,

	/// The tamper-evident, hash-chained audit log of agent actions (§IV-D).
	AuditLog = 8,

	/// Per-call agent approval state, binding a tool call's execution to its
	/// human-in-the-loop decision (§IV-C).
	AgentApprovals = 9,

	/// Provisioned agent identities keyed by user id (§IV-A).
	AgentRegistry = 10,
}

impl Domain {
	/// Every domain, in declaration order. Backends open one column family per
	/// entry.
	pub const ALL: [Self; 11] = [
		Self::Events,
		Self::RoomState,
		Self::AuthChainIndex,
		Self::ResolvedStateCache,
		Self::MediaMetadata,
		Self::DeviceStore,
		Self::KeyStore,
		Self::AccountData,
		Self::AuditLog,
		Self::AgentApprovals,
		Self::AgentRegistry,
	];

	/// The stable on-disk column-family name for this domain.
	///
	/// These strings are part of the storage contract and must not change
	/// without a migration.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			| Self::Events => "events",
			| Self::RoomState => "room_state",
			| Self::AuthChainIndex => "auth_chain_index",
			| Self::ResolvedStateCache => "resolved_state_cache",
			| Self::MediaMetadata => "media_metadata",
			| Self::DeviceStore => "device_store",
			| Self::KeyStore => "key_store",
			| Self::AccountData => "account_data",
			| Self::AuditLog => "audit_log",
			| Self::AgentApprovals => "agent_approvals",
			| Self::AgentRegistry => "agent_registry",
		}
	}

	/// The stable index of this domain, suitable for array-backed dispatch.
	#[must_use]
	pub const fn index(self) -> usize {
		match self {
			| Self::Events => 0,
			| Self::RoomState => 1,
			| Self::AuthChainIndex => 2,
			| Self::ResolvedStateCache => 3,
			| Self::MediaMetadata => 4,
			| Self::DeviceStore => 5,
			| Self::KeyStore => 6,
			| Self::AccountData => 7,
			| Self::AuditLog => 8,
			| Self::AgentApprovals => 9,
			| Self::AgentRegistry => 10,
		}
	}

	/// Resolve a domain from its on-disk column-family name.
	#[must_use]
	pub fn from_name(name: &str) -> Option<Self> {
		Self::ALL.into_iter().find(|domain| domain.name() == name)
	}
}

impl fmt::Display for Domain {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.name()) }
}
