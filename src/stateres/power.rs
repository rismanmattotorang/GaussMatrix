//! The power-levels content model used by the authorisation rules.
//!
//! This is a storage-pure projection of an `m.room.power_levels` event's
//! content: the per-user and default sender levels, and the levels required to
//! send events. A caller (a test, or the future `ruma` adapter) parses the event
//! content into this struct.

use std::collections::BTreeMap;

/// The relevant fields of an `m.room.power_levels` event.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PowerLevels {
	/// Explicit per-user power levels.
	pub users: BTreeMap<String, i64>,

	/// The power level of users not listed in `users`.
	pub users_default: i64,

	/// Explicit per-event-type power levels required to send them.
	pub events: BTreeMap<String, i64>,

	/// The level required to send a non-state event without an explicit entry.
	pub events_default: i64,

	/// The level required to send a state event without an explicit entry.
	pub state_default: i64,

	/// The level required to invite a user.
	pub invite: i64,

	/// The level required to kick a user.
	pub kick: i64,

	/// The level required to ban a user.
	pub ban: i64,
}

impl PowerLevels {
	/// The power level of `user`: their explicit level, or `users_default`.
	#[must_use]
	pub fn for_user(&self, user: &str) -> i64 {
		self.users.get(user).copied().unwrap_or(self.users_default)
	}

	/// The power level required to send an event of `event_type`: its explicit
	/// entry, or `state_default` / `events_default` depending on `is_state`.
	#[must_use]
	pub fn required_for(&self, event_type: &str, is_state: bool) -> i64 {
		let fallback = if is_state { self.state_default } else { self.events_default };
		self.events.get(event_type).copied().unwrap_or(fallback)
	}
}
