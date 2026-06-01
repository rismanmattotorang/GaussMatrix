//! # gm-fed — GaussMatrix federation sender (per-destination delivery)
//!
//! The federation sender is a pool rather than a single task
//! ([`GaussMatrix-SPECS.pdf`], §III-E): outbound transactions are sharded by
//! destination server so that one slow or unreachable peer cannot
//! head-of-line-block delivery to healthy peers — a direct improvement over the
//! single-process Conduit-family sender.
//!
//! This crate provides the storage- and transport-pure scheduling core:
//! [`FederationSender`] holds a per-destination outbound queue and an
//! independent exponential-backoff state per destination, so a failing
//! destination backs off on its own timeline while healthy destinations keep
//! flowing. Time is supplied by the caller (milliseconds) so the scheduler is
//! deterministic and testable; the async transport and signing build on it.
//!
//! [`GaussMatrix-SPECS.pdf`]: ../../GaussMatrix-SPECS.pdf

#![forbid(unsafe_code)]

mod sender;
#[cfg(test)]
mod tests;

pub use self::sender::{Destination, FederationSender};
