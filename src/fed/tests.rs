//! Tests for per-destination federation scheduling.

use crate::FederationSender;

#[test]
fn queue_and_take_round_trip() {
	let mut sender = FederationSender::new();
	sender.queue("a.example.org", b"txn1".to_vec());
	sender.queue("a.example.org", b"txn2".to_vec());
	assert_eq!(sender.pending("a.example.org"), 2);

	let taken = sender.take("a.example.org");
	assert_eq!(taken, vec![b"txn1".to_vec(), b"txn2".to_vec()]);
	assert_eq!(sender.pending("a.example.org"), 0);
}

#[test]
fn a_failing_destination_does_not_block_healthy_ones() {
	let mut sender = FederationSender::new();
	sender.queue("slow.example.org", b"a".to_vec());
	sender.queue("healthy.example.org", b"b".to_vec());

	// The slow destination fails and enters backoff.
	sender.mark_failure("slow.example.org", 1_000);

	// At t=1000 the healthy destination is ready; the slow one is not.
	let ready = sender.ready(1_000);
	assert!(ready.contains(&"healthy.example.org".to_owned()));
	assert!(!ready.contains(&"slow.example.org".to_owned()));

	// The healthy destination is delivered independently.
	assert_eq!(sender.take("healthy.example.org"), vec![b"b".to_vec()]);
}

#[test]
fn backoff_elapses_and_grows_with_repeated_failure() {
	let mut sender = FederationSender::new();
	sender.queue("peer.example.org", b"x".to_vec());

	// First failure at t=1000 → 1s backoff → ready again at t=2000.
	sender.mark_failure("peer.example.org", 1_000);
	assert!(sender.ready(1_999).is_empty());
	assert!(sender.ready(2_000).contains(&"peer.example.org".to_owned()));

	// A second consecutive failure backs off longer (2s).
	sender.mark_failure("peer.example.org", 2_000);
	assert!(sender.ready(3_999).is_empty());
	assert!(sender.ready(4_000).contains(&"peer.example.org".to_owned()));
}

#[test]
fn success_clears_backoff() {
	let mut sender = FederationSender::new();
	sender.queue("peer.example.org", b"x".to_vec());
	sender.mark_failure("peer.example.org", 1_000);
	assert!(sender.ready(1_000).is_empty());

	sender.mark_success("peer.example.org");
	// With backoff cleared, the destination is immediately ready again.
	assert!(sender.ready(1_000).contains(&"peer.example.org".to_owned()));
}

#[test]
fn empty_destinations_are_not_ready() {
	let mut sender = FederationSender::new();
	sender.queue("peer.example.org", b"x".to_vec());
	sender.take("peer.example.org");
	assert!(sender.ready(10_000).is_empty(), "a drained destination has nothing to send");
}

#[test]
fn queue_depths_reports_nonempty_destinations() {
	let mut sender = FederationSender::new();
	sender.queue("a.example.org", b"1".to_vec());
	sender.queue("a.example.org", b"2".to_vec());
	sender.queue("b.example.org", b"3".to_vec());

	let mut depths = sender.queue_depths();
	depths.sort();
	assert_eq!(depths, vec![
		("a.example.org".to_owned(), 2),
		("b.example.org".to_owned(), 1),
	]);

	// A drained destination drops out of the report.
	sender.take("a.example.org");
	assert_eq!(sender.queue_depths(), vec![("b.example.org".to_owned(), 1)]);
}

#[test]
fn backoff_state_reports_failing_destinations() {
	let mut sender = FederationSender::new();
	sender.queue("flaky.example.org", b"x".to_vec());

	// Healthy: no backoff state.
	assert!(sender.backoff_state(0).is_empty());

	// After a failure at t=1000, the destination is in backoff (1s) at t=1000.
	sender.mark_failure("flaky.example.org", 1_000);
	let state = sender.backoff_state(1_000);
	assert_eq!(state, vec![("flaky.example.org".to_owned(), 1, 2_000)]);

	// Once the window elapses it is no longer reported as failing.
	assert!(sender.backoff_state(2_000).is_empty());

	// Success clears it entirely.
	sender.mark_failure("flaky.example.org", 3_000);
	sender.mark_success("flaky.example.org");
	assert!(sender.backoff_state(3_000).is_empty());
}
