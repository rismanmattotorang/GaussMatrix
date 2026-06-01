//! Per-destination outbound scheduling with independent backoff.

use std::collections::{BTreeMap, VecDeque};

/// A destination server name (e.g. `matrix.example.org`).
pub type Destination = String;

/// Base and cap for the exponential per-destination retry backoff, in
/// milliseconds.
const BASE_BACKOFF_MS: u64 = 1_000;
const CAP_BACKOFF_MS: u64 = 60_000;

/// A destination's retry state: how many consecutive failures, and the time
/// before which it must not be retried.
#[derive(Clone, Copy, Debug, Default)]
struct Backoff {
	attempt: u32,
	available_at: u64,
}

/// Schedules outbound federation traffic per destination so a slow or
/// unreachable peer cannot block delivery to healthy peers.
#[derive(Clone, Debug, Default)]
pub struct FederationSender {
	queues: BTreeMap<Destination, VecDeque<Vec<u8>>>,
	backoff: BTreeMap<Destination, Backoff>,
}

impl FederationSender {
	/// A sender with no queued traffic.
	#[must_use]
	pub fn new() -> Self { Self::default() }

	/// Queue an outbound item (a serialised transaction/PDU) for `destination`.
	pub fn queue(&mut self, destination: &str, item: Vec<u8>) {
		self.queues.entry(destination.to_owned()).or_default().push_back(item);
	}

	/// The number of items queued for `destination`.
	#[must_use]
	pub fn pending(&self, destination: &str) -> usize {
		self.queues.get(destination).map_or(0, VecDeque::len)
	}

	/// The destinations that have queued traffic and are not in backoff at
	/// `now` — those ready to send, independent of any backed-off peer.
	#[must_use]
	pub fn ready(&self, now: u64) -> Vec<Destination> {
		self.queues
			.iter()
			.filter(|(_, queue)| !queue.is_empty())
			.filter(|(destination, _)| now >= self.available_at(destination))
			.map(|(destination, _)| destination.clone())
			.collect()
	}

	/// Take all currently-queued items for `destination` to send, clearing its
	/// queue.
	pub fn take(&mut self, destination: &str) -> Vec<Vec<u8>> {
		self.queues
			.remove(destination)
			.map(Vec::from_iter)
			.unwrap_or_default()
	}

	/// Record a successful delivery to `destination`, clearing its backoff.
	pub fn mark_success(&mut self, destination: &str) {
		self.backoff.remove(destination);
	}

	/// Record a failed delivery to `destination`, extending its backoff. The
	/// destination will not appear in [`ready`](Self::ready) until the backoff
	/// elapses; other destinations are unaffected.
	pub fn mark_failure(&mut self, destination: &str, now: u64) {
		let state = self.backoff.entry(destination.to_owned()).or_default();
		state.attempt = state.attempt.saturating_add(1);
		state.available_at = now.saturating_add(backoff_delay(state.attempt));
	}

	/// The time before which `destination` must not be retried (0 if healthy).
	fn available_at(&self, destination: &str) -> u64 {
		self.backoff.get(destination).map_or(0, |state| state.available_at)
	}
}

/// Exponential backoff for the `attempt`-th consecutive failure, capped.
fn backoff_delay(attempt: u32) -> u64 {
	let factor = 1_u64
		.checked_shl(attempt.saturating_sub(1).min(16))
		.unwrap_or(u64::MAX);

	BASE_BACKOFF_MS.saturating_mul(factor).min(CAP_BACKOFF_MS)
}
