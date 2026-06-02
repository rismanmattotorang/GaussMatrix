//! The federation scheduling service — a live, durable per-destination outbound
//! scheduler (SPECS §V).
//!
//! Wraps the transport-pure [`gm_fed::FederationSender`] behind an async mutex
//! so the running server holds one shared, thread-safe scheduler: traffic is
//! queued per destination with independent exponential backoff, so a slow or
//! unreachable peer never blocks delivery to healthy peers. The service owns the
//! clock, so callers record outcomes without managing time.
//!
//! Per-destination **health (backoff) is durable**: it is persisted to the
//! [`Domain::FederationHealth`] gm-store column family on every change and
//! restored on startup, so the scheduler's view survives restarts. The queued
//! in-flight markers are ephemeral (shadow mode).
//!
//! The production `sending` service runs the scheduler in **shadow mode**: it
//! enqueues an in-flight marker per federation transaction and mirrors the
//! outcome here (drain + [`mark_success`](Service::mark_success) on delivery,
//! [`mark_failure`](Service::mark_failure) on error), giving a live
//! per-destination view while the durable send path stays authoritative. Making
//! the scheduler drive delivery is the remaining cutover.

use std::{
	sync::Arc,
	time::{SystemTime, UNIX_EPOCH},
};

use gaussmatrix_core::{Result, err, implement};
use gm_fed::{Destination, FederationSender};
use gm_store::{Domain, DynStore};
use tokio::sync::Mutex;

pub struct Service {
	sender: Mutex<FederationSender>,
	store: DynStore,
}

impl crate::Service for Service {
	fn build(args: &crate::Args<'_>) -> Result<Arc<Self>> {
		let store = args.store.clone();
		let mut sender = FederationSender::new();
		restore_health(&store, &mut sender)?;

		Ok(Arc::new(Self { sender: Mutex::new(sender), store }))
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

/// Record a successful delivery to `destination`, clearing its backoff (and its
/// persisted health record).
#[implement(Service)]
pub async fn mark_success(&self, destination: &str) {
	self.sender.lock().await.mark_success(destination);
	if let Err(e) = self.store.delete(Domain::FederationHealth, destination.as_bytes().to_vec()) {
		gaussmatrix_core::warn!("failed to clear federation health for {destination}: {e}");
	}
}

/// Record a failed delivery to `destination`, extending its backoff and
/// persisting the new health record.
#[implement(Service)]
pub async fn mark_failure(&self, destination: &str) {
	let persisted = {
		let mut sender = self.sender.lock().await;
		sender.mark_failure(destination, now_millis());
		sender.backoff_for(destination)
	};

	if let Some((attempt, available_at)) = persisted
		&& let Err(e) = self.store.put(
			Domain::FederationHealth,
			destination.as_bytes().to_vec(),
			encode_health(attempt, available_at),
		) {
		gaussmatrix_core::warn!("failed to persist federation health for {destination}: {e}");
	}
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

/// Load persisted per-destination health into `sender` on startup.
fn restore_health(store: &DynStore, sender: &mut FederationSender) -> Result<()> {
	let scanned = store
		.prefix_scan(Domain::FederationHealth, b"")
		.map_err(|e| err!(Database("federation health scan failed: {e}")))?;

	for (key, value) in scanned {
		if let (Ok(destination), Some((attempt, available_at))) =
			(String::from_utf8(key.into_vec()), decode_health(&value))
		{
			sender.restore(&destination, attempt, available_at);
		}
	}

	Ok(())
}

/// Encode a health record as `attempt_be(4) || available_at_be(8)`.
fn encode_health(attempt: u32, available_at: u64) -> Vec<u8> {
	let mut value = Vec::with_capacity(12);
	value.extend_from_slice(&attempt.to_be_bytes());
	value.extend_from_slice(&available_at.to_be_bytes());
	value
}

/// Decode a health record written by [`encode_health`].
fn decode_health(bytes: &[u8]) -> Option<(u32, u64)> {
	let attempt = <[u8; 4]>::try_from(bytes.get(0..4)?).ok()?;
	let available_at = <[u8; 8]>::try_from(bytes.get(4..12)?).ok()?;
	Some((u32::from_be_bytes(attempt), u64::from_be_bytes(available_at)))
}

/// Milliseconds since the Unix epoch (saturating).
fn now_millis() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.ok()
		.and_then(|d| u64::try_from(d.as_millis()).ok())
		.unwrap_or(0)
}
