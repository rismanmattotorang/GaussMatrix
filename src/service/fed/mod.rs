//! The federation scheduling service — a live per-destination outbound
//! scheduler (SPECS §V).
//!
//! Wraps the transport-pure [`gm_fed::FederationSender`] behind an async mutex
//! so the running server holds one shared, thread-safe scheduler: traffic is
//! queued per destination with independent exponential backoff, so a slow or
//! unreachable peer never blocks delivery to healthy peers. The service owns the
//! clock, so callers record outcomes without managing time.
//!
//! The production `sending` service mirrors real federation transaction outcomes
//! here ([`mark_success`](Service::mark_success) / [`mark_failure`](Service::mark_failure)),
//! giving a live per-destination health view; routing the outbound path itself
//! through the scheduler is the remaining cutover.

use std::{
	sync::Arc,
	time::{SystemTime, UNIX_EPOCH},
};

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

/// Take all currently-queued items for `destination`, clearing its queue.
#[implement(Service)]
pub async fn take(&self, destination: &str) -> Vec<Vec<u8>> {
	self.sender.lock().await.take(destination)
}

/// The destinations with queued traffic that are not in backoff now.
#[implement(Service)]
pub async fn ready(&self) -> Vec<Destination> {
	self.sender.lock().await.ready(now_millis())
}

/// Record a successful delivery to `destination`, clearing its backoff.
#[implement(Service)]
pub async fn mark_success(&self, destination: &str) {
	self.sender.lock().await.mark_success(destination);
}

/// Record a failed delivery to `destination`, extending its backoff.
#[implement(Service)]
pub async fn mark_failure(&self, destination: &str) {
	self.sender.lock().await.mark_failure(destination, now_millis());
}

/// A snapshot of every destination with queued traffic and its queue depth.
#[implement(Service)]
pub async fn queue_depths(&self) -> Vec<(Destination, usize)> {
	self.sender.lock().await.queue_depths()
}

/// Destinations currently in backoff: each with its consecutive-failure count
/// and the time (epoch ms) it becomes available again — the live health view.
#[implement(Service)]
pub async fn failing(&self) -> Vec<(Destination, u32, u64)> {
	self.sender.lock().await.backoff_state(now_millis())
}

/// Milliseconds since the Unix epoch (saturating).
fn now_millis() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.ok()
		.and_then(|d| u64::try_from(d.as_millis()).ok())
		.unwrap_or(0)
}
