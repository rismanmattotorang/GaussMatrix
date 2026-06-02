//! The federation scheduling service — a live per-destination outbound
//! scheduler (SPECS §V).
//!
//! Wraps the transport-pure [`gm_fed::FederationSender`] behind an async mutex
//! so the running server holds one shared, thread-safe scheduler: traffic is
//! queued per destination with independent exponential backoff, so a slow or
//! unreachable peer never blocks delivery to healthy peers.
//!
//! This is the additive seam for the distributed sender (the same staging the
//! other `gm-*` crates use): the scheduler is live and inspectable now, and the
//! cutover that routes the production outbound path through it is a follow-up.

use std::sync::Arc;

use gaussmatrix_core::{Result, implement};
use gm_fed::{Destination, FederationSender};
use tokio::sync::Mutex;

pub struct Service {
	sender: Mutex<FederationSender>,
}

impl crate::Service for Service {
	fn build(_args: &crate::Args<'_>) -> Result<Arc<Self>> {
		Ok(Arc::new(Self { sender: Mutex::new(FederationSender::new()) }))
	}

	fn name(&self) -> &str { crate::service::make_name(std::module_path!()) }
}

/// Queue a serialised outbound `item` for `destination`.
#[implement(Service)]
pub async fn queue(&self, destination: &str, item: Vec<u8>) {
	self.sender.lock().await.queue(destination, item);
}

/// The number of items queued for `destination`.
#[implement(Service)]
pub async fn pending(&self, destination: &str) -> usize {
	self.sender.lock().await.pending(destination)
}

/// The destinations with queued traffic that are not in backoff at `now`.
#[implement(Service)]
pub async fn ready(&self, now: u64) -> Vec<Destination> {
	self.sender.lock().await.ready(now)
}

/// Take all currently-queued items for `destination`, clearing its queue.
#[implement(Service)]
pub async fn take(&self, destination: &str) -> Vec<Vec<u8>> {
	self.sender.lock().await.take(destination)
}

/// Record a successful delivery to `destination`, clearing its backoff.
#[implement(Service)]
pub async fn mark_success(&self, destination: &str) {
	self.sender.lock().await.mark_success(destination);
}

/// Record a failed delivery to `destination`, extending its backoff.
#[implement(Service)]
pub async fn mark_failure(&self, destination: &str, now: u64) {
	self.sender.lock().await.mark_failure(destination, now);
}

/// A snapshot of every destination with queued traffic and its queue depth.
#[implement(Service)]
pub async fn queue_depths(&self) -> Vec<(Destination, usize)> {
	self.sender.lock().await.queue_depths()
}
