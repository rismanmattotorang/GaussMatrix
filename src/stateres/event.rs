//! The event projection the resolution ordering operates on.

use crate::{power::PowerLevels, state_map::EventId};

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

	/// This event's type (e.g. `m.room.power_levels`).
	fn event_type(&self) -> &str;

	/// This event's state key. State resolution only ever resolves state events,
	/// so this is always present (the empty string for state events without a
	/// meaningful key, such as `m.room.power_levels`).
	fn state_key(&self) -> &str;

	/// The effective power level of this event's sender, as determined by the
	/// authorisation state against which the event is being ordered.
	fn power_level(&self) -> i64;

	/// The event's `origin_server_ts`.
	fn origin_server_ts(&self) -> u64;

	/// The identifiers of this event's auth events.
	fn auth_event_ids(&self) -> &[EventId];

	/// The user id of this event's sender.
	fn sender(&self) -> &str;

	/// The parsed power-levels content, present only for `m.room.power_levels`
	/// events.
	fn power_levels(&self) -> Option<&PowerLevels>;

	/// The membership (`join`/`invite`/`leave`/`ban`/`knock`), present only for
	/// `m.room.member` events.
	fn membership(&self) -> Option<&str>;

	/// The join rule (`public`/`invite`/…), present only for
	/// `m.room.join_rules` events.
	fn join_rule(&self) -> Option<&str>;
}
