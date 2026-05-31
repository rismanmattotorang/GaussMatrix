//! The event projection the resolution ordering operates on.

use crate::state_map::EventId;

/// The minimal projection of a Matrix event that the resolution ordering needs.
///
/// The engine is pure with respect to storage and the wire format: a caller
/// supplies these fields for each event (read from the store and the resolved
/// auth state) rather than the engine reaching for them. This keeps ordering
/// deterministic and unit-testable, and defers the `ruma`/`gm-api` event types
/// to the point where the engine is wired into the server.
pub trait Event {
	/// This event's identifier.
	fn event_id(&self) -> &str;

	/// The effective power level of this event's sender, as determined by the
	/// authorisation state against which the event is being ordered.
	fn power_level(&self) -> i64;

	/// The event's `origin_server_ts`.
	fn origin_server_ts(&self) -> u64;

	/// The identifiers of this event's auth events.
	fn auth_event_ids(&self) -> &[EventId];
}
