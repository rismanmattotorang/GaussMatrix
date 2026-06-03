//! The federation scheduling service — a live, durable per-destination outbound
//! scheduler (SPECS §V).
//!
//! Wraps the transport-pure [`gm_fed::FederationSender`] (which holds the
//! per-destination backoff/health) and backs the **outbound queue with
//! gm-store**, so both health and queued traffic survive restarts — the basis
//! for an authoritative scheduler. The service owns the clock, so callers record
//! outcomes without managing time.
//!
//! Health (backoff) is persisted to [`Domain::FederationHealth`]; the outbound
//! queue is persisted to [`Domain::FederationQueue`], keyed `destination \0
//! seq_be` so per-destination ordering is preserved and a restart resumes where
//! it left off.
//!
//! The production `sending` service runs the scheduler in **shadow mode**: it
//! enqueues an in-flight marker per federation transaction and mirrors the
//! outcome here (drain + [`mark_success`](Service::mark_success) on delivery,
//! [`mark_failure`](Service::mark_failure) on error), giving a live, restart-safe
//! per-destination view while the durable send path stays authoritative. Making
//! the scheduler itself drive delivery is the remaining cutover.

use std::{
	collections::{BTreeMap, BTreeSet},
	sync::{
		Arc,
		atomic::{AtomicU64, Ordering},
	},
	time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use futures::FutureExt;
use gaussmatrix_core::{Result, debug, err, implement, utils::calculate_hash, warn};
use gm_fed::{Destination, FederationSender};
use gm_store::{Domain, DynStore};
use ruma::{
	CanonicalJsonObject, MilliSecondsSinceUnixEpoch, OwnedServerName, ServerName,
	api::federation::transactions::{edu::Edu, send_transaction_message},
	serde::Raw,
};
use serde_json::value::RawValue as RawJsonValue;
use tokio::sync::Mutex;

/// How often the gated scheduler drive loop wakes to flush ready destinations.
/// Only does work when `gm_fed_authoritative_sender` is enabled.
const SCHEDULER_DRIVE_INTERVAL: Duration = Duration::from_secs(30);

pub struct Service {
	sender: Mutex<FederationSender>,
	store: DynStore,
	seq: AtomicU64,
	services: Arc<crate::services::OnceServices>,
}

#[async_trait]
impl crate::Service for Service {
	fn build(args: &crate::Args<'_>) -> Result<Arc<Self>> {
		let store = args.store.clone();
		let mut sender = FederationSender::new();
		restore_health(&store, &mut sender)?;
		let next_seq = max_queue_seq(&store)?.saturating_add(1);

		Ok(Arc::new(Self {
			sender: Mutex::new(sender),
			store,
			seq: AtomicU64::new(next_seq),
			services: args.services.clone(),
		}))
	}

	/// Gated periodic drive loop. Sleeps between cycles and, only when
	/// `gm_fed_authoritative_sender` is enabled, runs one [`drive_once`] cycle
	/// per tick — gm-fed schedules ready destinations and the existing sender
	/// transports them. Default-off, so this is a no-op loop in production until
	/// explicitly enabled.
	async fn worker(self: Arc<Self>) -> Result {
		loop {
			tokio::select! {
				() = tokio::time::sleep(SCHEDULER_DRIVE_INTERVAL) => {},
				() = self.services.server.until_shutdown() => return Ok(()),
			}

			if !self.services.server.config.gm_fed_authoritative_sender {
				continue;
			}

			match self.drive_once().await {
				| Ok(0) => {},
				| Ok(count) => debug!("gm-fed scheduler drove {count} destination(s)"),
				| Err(error) => warn!("gm-fed scheduler drive failed: {error}"),
			}
		}
	}

	fn name(&self) -> &str { crate::service::make_name(std::module_path!()) }
}

/// Durably queue a serialised outbound `item` for `destination`. Best-effort on
/// the shadow path: a store error is logged, not propagated.
#[implement(Service)]
pub fn queue(&self, destination: &str, item: Vec<u8>) {
	let seq = self.seq.fetch_add(1, Ordering::Relaxed);
	if let Err(e) =
		self.store.put(Domain::FederationQueue, queue_key(destination, seq), item)
	{
		warn!("failed to durably queue federation item for {destination}: {e}");
	}
}

/// The number of items durably queued for `destination`.
#[implement(Service)]
pub fn pending(&self, destination: &str) -> usize {
	queue_depth(&self.store, destination).unwrap_or(0)
}

/// Take (and delete) all durably-queued items for `destination`, in order.
#[implement(Service)]
pub fn take(&self, destination: &str) -> Vec<Vec<u8>> {
	drain_queue(&self.store, destination).unwrap_or_else(|e| {
		warn!("failed to drain federation queue for {destination}: {e}");
		Vec::new()
	})
}

/// The destinations with durably-queued traffic that are not in backoff now.
#[implement(Service)]
pub async fn ready(&self) -> Vec<Destination> {
	let backed_off: BTreeSet<Destination> = {
		let sender = self.sender.lock().await;
		sender.backoff_state(now_millis()).into_iter().map(|(dest, ..)| dest).collect()
	};

	all_queue_depths(&self.store)
		.unwrap_or_default()
		.into_iter()
		.filter(|(dest, depth)| *depth > 0 && !backed_off.contains(dest))
		.map(|(dest, _)| dest)
		.collect()
}

/// Drain every destination that is ready now, returning the batches due for
/// delivery — the scheduling drive (queue → ready → take) composed in one step.
///
/// This is the core an authoritative sender calls each tick: it selects the
/// ready destinations (queued and not in backoff) and atomically removes their
/// items. Binding the returned batches to the federation transport is the
/// remaining cutover; until then the production path delivers and this drains
/// the shadow queue.
#[implement(Service)]
pub async fn tick(&self) -> Vec<(Destination, Vec<Vec<u8>>)> {
	let backed_off: BTreeSet<Destination> = {
		let sender = self.sender.lock().await;
		sender.backoff_state(now_millis()).into_iter().map(|(dest, ..)| dest).collect()
	};

	due_batches(&self.store, &backed_off).unwrap_or_else(|e| {
		warn!("federation scheduler tick failed: {e}");
		Vec::new()
	})
}

/// Drive one scheduling cycle through gm-fed's **native transport**
/// (config-gated): select the ready destinations, drain their durable queue,
/// and for each build a federation transaction and dispatch it ourselves —
/// reusing the proven request signing and HTTP client via
/// [`federation::execute_on`](crate::federation::Service::execute_on) — then
/// record the real outcome (success clears backoff; failure schedules it).
/// Returns the number of destinations driven.
///
/// gm-fed owns the construct → sign → send → record loop here, rather than
/// delegating to the legacy sender's queue. The durable queue items are
/// serialized PDU JSON; today the production path enqueues shadow markers, which
/// parse to no PDUs and make each dispatch a safe no-op, so the native path is
/// inert until gm-fed's queue is fed real outbound PDUs.
///
/// Gated on `gm_fed_authoritative_sender` — disabled by default.
///
/// # Errors
///
/// Returns an error if the feature is disabled.
#[implement(Service)]
pub async fn drive_once(&self) -> Result<usize> {
	if !self.services.server.config.gm_fed_authoritative_sender {
		return Err(err!(
			"gm-fed authoritative sender is disabled; set gm_fed_authoritative_sender = true"
		));
	}

	let batches = self.tick().await;
	let mut driven: usize = 0;

	for (dest, items) in batches {
		let Ok(server) = OwnedServerName::parse(&dest) else {
			warn!("federation scheduler: skipping invalid destination {dest:?}");
			continue;
		};

		// gm-fed's durable queue items are serialized PDU JSON. Shadow markers
		// (empty or non-PDU bytes) parse to nothing, leaving an empty
		// transaction that `dispatch_transaction` reports as a no-op.
		let pdus: Vec<CanonicalJsonObject> = items
			.iter()
			.filter_map(|item| serde_json::from_slice(item).ok())
			.collect();

		match self.dispatch_transaction(&server, pdus).await {
			| Ok(_) => self.mark_success(server.as_str()).await,
			| Err(error) => {
				warn!("federation scheduler: native dispatch to {server} failed: {error}");
				self.mark_failure(server.as_str()).await;
			},
		}

		driven = driven.saturating_add(1);
	}

	Ok(driven)
}

/// Build a federation transaction from the resolved PDUs and dispatch it to
/// `dest` over the proven federation transport (which signs the request with the
/// server's Ed25519 key and sends it via the federation HTTP client). Returns
/// `false` without a network call when there is nothing to send.
#[implement(Service)]
async fn dispatch_transaction(
	&self,
	dest: &ServerName,
	pdu_jsons: Vec<CanonicalJsonObject>,
) -> Result<bool> {
	if pdu_jsons.is_empty() {
		return Ok(false);
	}

	let mut pdus: Vec<Box<RawJsonValue>> = Vec::with_capacity(pdu_jsons.len());
	for pdu_json in pdu_jsons {
		// `.boxed()` type-erases the deeply-nested federation futures; left
		// inline their layout depth propagates up through the drive loop's
		// `worker` async block and overflows the layout recursion limit (the
		// same reason the legacy sender `.boxed()`s its send futures).
		pdus.push(self.services.federation.format_pdu_into(pdu_json, None).boxed().await);
	}
	// EDU multiplexing stays with the legacy sender for now; the native path
	// ships PDUs only.
	let edus: Vec<Raw<Edu>> = Vec::new();

	let request = build_transaction(self.services.server.name.clone(), pdus, edus);
	let txn_id = request.transaction_id.clone();
	debug!(%dest, %txn_id, pdus = request.pdus.len(), "Dispatching native federation transaction");

	self.services
		.federation
		.execute_on(&self.services.client.sender, dest, request)
		.boxed()
		.await?;

	Ok(true)
}

/// Record a successful delivery to `destination`, clearing its backoff (and its
/// persisted health record).
#[implement(Service)]
pub async fn mark_success(&self, destination: &str) {
	self.sender.lock().await.mark_success(destination);
	if let Err(e) = self.store.delete(Domain::FederationHealth, destination.as_bytes().to_vec()) {
		warn!("failed to clear federation health for {destination}: {e}");
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
		warn!("failed to persist federation health for {destination}: {e}");
	}
}

/// A snapshot of every destination with durably-queued traffic and its depth.
#[implement(Service)]
pub fn queue_depths(&self) -> Vec<(Destination, usize)> {
	all_queue_depths(&self.store).unwrap_or_default()
}

/// Destinations currently in backoff: each with its consecutive-failure count
/// and the time (epoch ms) it becomes available again — the live health view.
#[implement(Service)]
pub async fn failing(&self) -> Vec<(Destination, u32, u64)> {
	self.sender.lock().await.backoff_state(now_millis())
}

/// The [`Domain::FederationQueue`] key for an item: `destination \0 seq_be`.
fn queue_key(destination: &str, seq: u64) -> Vec<u8> {
	let mut key = Vec::with_capacity(destination.len().saturating_add(9));
	key.extend_from_slice(destination.as_bytes());
	key.push(0);
	key.extend_from_slice(&seq.to_be_bytes());
	key
}

/// The key prefix scanning all of `destination`'s queued items.
fn queue_prefix(destination: &str) -> Vec<u8> {
	let mut prefix = Vec::with_capacity(destination.len().saturating_add(1));
	prefix.extend_from_slice(destination.as_bytes());
	prefix.push(0);
	prefix
}

/// The number of items queued for `destination`.
fn queue_depth(store: &DynStore, destination: &str) -> Result<usize> {
	store
		.prefix_scan(Domain::FederationQueue, &queue_prefix(destination))
		.map(|scanned| scanned.len())
		.map_err(|e| err!(Database("federation queue scan failed: {e}")))
}

/// Take and delete all of `destination`'s queued items, in sequence order.
fn drain_queue(store: &DynStore, destination: &str) -> Result<Vec<Vec<u8>>> {
	let scanned = store
		.prefix_scan(Domain::FederationQueue, &queue_prefix(destination))
		.map_err(|e| err!(Database("federation queue scan failed: {e}")))?;

	let mut items = Vec::with_capacity(scanned.len());
	for (key, value) in scanned {
		store
			.delete(Domain::FederationQueue, key)
			.map_err(|e| err!(Database("federation queue delete failed: {e}")))?;
		items.push(value);
	}

	Ok(items)
}

/// Queue depth per destination across the whole [`Domain::FederationQueue`].
fn all_queue_depths(store: &DynStore) -> Result<Vec<(Destination, usize)>> {
	let scanned = store
		.prefix_scan(Domain::FederationQueue, b"")
		.map_err(|e| err!(Database("federation queue scan failed: {e}")))?;

	let mut depths: BTreeMap<Destination, usize> = BTreeMap::new();
	for (key, _value) in scanned {
		if let Some(sep) = key.iter().position(|&byte| byte == 0)
			&& let Ok(destination) = std::str::from_utf8(&key[..sep])
		{
			let counter = depths.entry(destination.to_owned()).or_default();
			*counter = counter.saturating_add(1);
		}
	}

	Ok(depths.into_iter().collect())
}

/// The batches due for delivery now: each ready destination (queued and not in
/// `backed_off`) drained in sequence order.
fn due_batches(
	store: &DynStore,
	backed_off: &BTreeSet<Destination>,
) -> Result<Vec<(Destination, Vec<Vec<u8>>)>> {
	let mut batches = Vec::new();
	for (destination, depth) in all_queue_depths(store)? {
		if depth > 0 && !backed_off.contains(&destination) {
			let items = drain_queue(store, &destination)?;
			if !items.is_empty() {
				batches.push((destination, items));
			}
		}
	}

	Ok(batches)
}

/// The greatest queue sequence currently persisted (0 if the queue is empty),
/// so a restart resumes assigning monotonically-increasing keys.
fn max_queue_seq(store: &DynStore) -> Result<u64> {
	let scanned = store
		.prefix_scan(Domain::FederationQueue, b"")
		.map_err(|e| err!(Database("federation queue scan failed: {e}")))?;

	let mut max = 0;
	for (key, _value) in scanned {
		let len = key.len();
		if len >= 8
			&& let Ok(bytes) = <[u8; 8]>::try_from(&key[len.saturating_sub(8)..])
		{
			max = max.max(u64::from_be_bytes(bytes));
		}
	}

	Ok(max)
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

/// The federation transaction id for a batch: the URL-safe base64 of a hash over
/// the (delimited) PDU and EDU bytes. Pure and content-addressed — the same
/// contents always yield the same id, so a retransmission is idempotent on the
/// receiving server. Mirrors the legacy sender's derivation.
fn transaction_id(pdus: &[Box<RawJsonValue>], edus: &[Raw<Edu>]) -> String {
	let preimage = pdus
		.iter()
		.map(|raw| raw.get().as_bytes())
		.chain(edus.iter().map(|raw| raw.json().get().as_bytes()));

	URL_SAFE_NO_PAD.encode(calculate_hash(preimage))
}

/// Assemble a `send_transaction_message` request from `origin` and the resolved
/// PDUs/EDUs, stamping it with a content-addressed [`transaction_id`] and the
/// current time. Pure but for the timestamp.
fn build_transaction(
	origin: OwnedServerName,
	pdus: Vec<Box<RawJsonValue>>,
	edus: Vec<Raw<Edu>>,
) -> send_transaction_message::v1::Request {
	let transaction_id = transaction_id(&pdus, &edus);

	send_transaction_message::v1::Request {
		transaction_id: transaction_id.as_str().into(),
		origin,
		origin_server_ts: MilliSecondsSinceUnixEpoch::now(),
		pdus,
		edus,
	}
}

#[cfg(test)]
mod tests {
	use std::collections::BTreeSet;

	use gm_store::{DynStore, MemBackend};

	use super::{
		Domain, Edu, Raw, all_queue_depths, drain_queue, due_batches, max_queue_seq, queue_depth,
		queue_key, transaction_id,
	};

	fn mem_store() -> DynStore { DynStore::new(MemBackend::default()) }

	#[test]
	fn transaction_id_is_content_addressed() {
		let raw = |v: serde_json::Value| serde_json::value::to_raw_value(&v).unwrap();
		let no_edus: Vec<Raw<Edu>> = Vec::new();

		let id = transaction_id(&[raw(serde_json::json!({ "a": 1 }))], &no_edus);

		// Stable, non-empty, and identical content yields the identical id (so a
		// retransmission is idempotent on the receiver).
		assert!(!id.is_empty());
		assert_eq!(id, transaction_id(&[raw(serde_json::json!({ "a": 1 }))], &no_edus));

		// Different PDU content yields a different id.
		assert_ne!(id, transaction_id(&[raw(serde_json::json!({ "a": 2 }))], &no_edus));
		// An empty batch is distinct from a populated one.
		assert_ne!(id, transaction_id(&[], &no_edus));
	}

	#[test]
	fn durable_queue_orders_and_drains_per_destination() {
		let store = mem_store();
		// Enqueue out of order across two destinations.
		store.put(Domain::FederationQueue, queue_key("a.org", 0), b"a0".to_vec()).unwrap();
		store.put(Domain::FederationQueue, queue_key("a.org", 1), b"a1".to_vec()).unwrap();
		store.put(Domain::FederationQueue, queue_key("b.org", 0), b"b0".to_vec()).unwrap();

		assert_eq!(queue_depth(&store, "a.org").unwrap(), 2);
		assert_eq!(queue_depth(&store, "b.org").unwrap(), 1);

		// Draining a.org returns its items in sequence order and clears them.
		let drained = drain_queue(&store, "a.org").unwrap();
		assert_eq!(drained, vec![b"a0".to_vec(), b"a1".to_vec()]);
		assert_eq!(queue_depth(&store, "a.org").unwrap(), 0);
		// b.org is unaffected — no cross-destination prefix collision.
		assert_eq!(queue_depth(&store, "b.org").unwrap(), 1);
	}

	#[test]
	fn depths_and_max_seq_span_all_destinations() {
		let store = mem_store();
		store.put(Domain::FederationQueue, queue_key("a.org", 3), b"x".to_vec()).unwrap();
		store.put(Domain::FederationQueue, queue_key("b.org", 7), b"y".to_vec()).unwrap();

		let depths = all_queue_depths(&store).unwrap();
		assert_eq!(depths, vec![("a.org".to_owned(), 1), ("b.org".to_owned(), 1)]);

		// Seq seeding resumes past the greatest persisted key.
		assert_eq!(max_queue_seq(&store).unwrap(), 7);
		assert_eq!(max_queue_seq(&mem_store()).unwrap(), 0);
	}

	#[test]
	fn tick_drains_only_ready_destinations() {
		let store = mem_store();
		store.put(Domain::FederationQueue, queue_key("ready.org", 0), b"r0".to_vec()).unwrap();
		store.put(Domain::FederationQueue, queue_key("ready.org", 1), b"r1".to_vec()).unwrap();
		store.put(Domain::FederationQueue, queue_key("backed.org", 0), b"b0".to_vec()).unwrap();

		// backed.org is in backoff, so the tick skips it and drains only ready.org.
		let backed_off: BTreeSet<_> = std::iter::once("backed.org".to_owned()).collect();
		let batches = due_batches(&store, &backed_off).unwrap();
		assert_eq!(batches, vec![("ready.org".to_owned(), vec![b"r0".to_vec(), b"r1".to_vec()])]);

		// The drained destination is now empty; the backed-off one is intact.
		assert_eq!(queue_depth(&store, "ready.org").unwrap(), 0);
		assert_eq!(queue_depth(&store, "backed.org").unwrap(), 1);
	}
}
