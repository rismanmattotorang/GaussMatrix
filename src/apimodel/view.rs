//! A backend-neutral view of a Matrix event's fields.
//!
//! Bridging a server PDU (or any event representation) into the resolution
//! model goes through [`EventView`]: something exposes an event's
//! already-extracted fields, and [`StateEvent::from_view`](crate::StateEvent::from_view)
//! builds the resolution event from them. The sender's effective power level is
//! *derived* during resolution, so it is supplied to `from_view` explicitly
//! rather than read from the event.
//!
//! The feature-gated `core-bridge` adapter provides a blanket `EventView` over
//! the server's `gaussmatrix_core::matrix::Event`, so a real PDU bridges into
//! the engine without a JSON round-trip.

use serde_json::Value;

/// A backend-neutral projection of a Matrix event's fields, as owned values.
pub trait EventView {
	/// The event id.
	fn event_id(&self) -> String;

	/// The event type (e.g. `m.room.member`).
	fn event_type(&self) -> String;

	/// The sender's user id.
	fn sender(&self) -> String;

	/// The state key, if this is a state event.
	fn state_key(&self) -> Option<String>;

	/// The `origin_server_ts` in milliseconds.
	fn origin_server_ts(&self) -> u64;

	/// The ids of this event's auth events.
	fn auth_event_ids(&self) -> Vec<String>;

	/// The event content as JSON.
	fn content(&self) -> Value;
}
