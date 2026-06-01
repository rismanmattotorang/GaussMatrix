//! # gm-shard — GaussMatrix room sharding (placement)
//!
//! The defining departure from the Conduit-family baseline is the sharded
//! profile ([`GaussMatrix-SPECS.pdf`], §III-F): stateless front-ends route each
//! request to the room-worker shard that owns the room, and each shard owns a
//! disjoint partition of rooms. This crate provides the placement primitive —
//! a [`ShardRing`] that maps a room to its owning shard by **consistent
//! hashing**, so adding or draining a shard reassigns only a small fraction of
//! rooms (the rest keep their owner, bounding the working sets that must be
//! warmed before cut-over).
//!
//! The ring is deterministic and portable: placement uses a fixed 64-bit FNV-1a
//! hash, so every front-end computes the same owner for a room without
//! coordination. The coordination service that maintains membership and
//! orchestrates online rebalancing builds on this primitive.
//!
//! [`GaussMatrix-SPECS.pdf`]: ../../GaussMatrix-SPECS.pdf

#![forbid(unsafe_code)]

mod ring;
#[cfg(test)]
mod tests;

pub use self::ring::{ShardId, ShardRing};
