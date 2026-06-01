//! A resolution event built from Matrix wire fields and parsed content.

use gm_stateres::{Event, EventId, PowerLevels};
use serde_json::Value;

use crate::{
	content::{
		join_authorised_from_content, join_rule_from_content, membership_from_content,
		power_levels_from_content,
	},
	view::EventView,
};

/// A state event carrying the projections [`gm_stateres`] needs, built from the
/// wire fields plus parsed content.
///
/// Built fluently: start from [`StateEvent::new`] and layer on the state key,
/// timestamp, auth events, content, and (when known) the sender's resolved
/// power level. Content is parsed according to the event type when set via
/// [`StateEvent::with_content`].
#[derive(Clone, Debug)]
pub struct StateEvent {
	event_id: EventId,
	event_type: String,
	state_key: String,
	sender: String,
	origin_server_ts: u64,
	auth_event_ids: Vec<EventId>,
	power_level: i64,
	power_levels: Option<PowerLevels>,
	membership: Option<String>,
	join_rule: Option<String>,
	join_authorised: Option<String>,
}

impl StateEvent {
	/// Begin a state event with the given id, type, and sender. Other fields
	/// default (empty state key, zero timestamp, no auth events, sender power
	/// level zero) until set.
	#[must_use]
	pub fn new(event_id: &str, event_type: &str, sender: &str) -> Self {
		Self {
			event_id: event_id.to_owned(),
			event_type: event_type.to_owned(),
			state_key: String::new(),
			sender: sender.to_owned(),
			origin_server_ts: 0,
			auth_event_ids: Vec::new(),
			power_level: 0,
			power_levels: None,
			membership: None,
			join_rule: None,
			join_authorised: None,
		}
	}

	/// Set the state key.
	#[must_use]
	pub fn with_state_key(mut self, state_key: &str) -> Self {
		state_key.clone_into(&mut self.state_key);
		self
	}

	/// Set the `origin_server_ts`.
	#[must_use]
	pub fn with_origin_server_ts(mut self, ts: u64) -> Self {
		self.origin_server_ts = ts;
		self
	}

	/// Set the auth event ids.
	#[must_use]
	pub fn with_auth_events(mut self, auth_event_ids: &[&str]) -> Self {
		self.auth_event_ids = auth_event_ids.iter().map(|id| (*id).to_owned()).collect();
		self
	}

	/// Set the sender's resolved power level (derived during resolution).
	#[must_use]
	pub fn with_power_level(mut self, power_level: i64) -> Self {
		self.power_level = power_level;
		self
	}

	/// Parse `content` according to the event type, populating the power-levels,
	/// membership, or join-rule projection as appropriate.
	#[must_use]
	pub fn with_content(mut self, content: &Value) -> Self {
		match self.event_type.as_str() {
			| "m.room.power_levels" =>
				self.power_levels = Some(power_levels_from_content(content)),
			| "m.room.member" => {
				self.membership = membership_from_content(content);
				self.join_authorised = join_authorised_from_content(content);
			},
			| "m.room.join_rules" => self.join_rule = join_rule_from_content(content),
			| _ => {},
		}
		self
	}

	/// Build a `StateEvent` from a canonical Matrix event JSON object and a
	/// separately-supplied `event_id` (which, from room version 3, is not carried
	/// in the event body but computed as a reference hash).
	///
	/// Returns `None` if a required field (`type`, `sender`, `origin_server_ts`)
	/// is missing or malformed. The `auth_events` array is accepted in both the
	/// room v1/v2 form (`[event_id, hashes]` pairs) and the v3+ form (event-id
	/// strings). The sender's resolved power level is not part of the event and
	/// is left at zero.
	#[must_use]
	pub fn from_event_json(event_id: &str, event: &Value) -> Option<Self> {
		let event_type = event.get("type")?.as_str()?;
		let sender = event.get("sender")?.as_str()?;
		let origin_server_ts = event.get("origin_server_ts")?.as_u64()?;

		let mut state =
			Self::new(event_id, event_type, sender).with_origin_server_ts(origin_server_ts);

		if let Some(state_key) = event.get("state_key").and_then(Value::as_str) {
			state = state.with_state_key(state_key);
		}

		let auth_events: Vec<&str> = event
			.get("auth_events")
			.and_then(Value::as_array)
			.map(|entries| entries.iter().filter_map(auth_event_id).collect())
			.unwrap_or_default();
		state = state.with_auth_events(&auth_events);

		if let Some(content) = event.get("content") {
			state = state.with_content(content);
		}

		Some(state)
	}
}

impl StateEvent {
	/// Build a `StateEvent` from an [`EventView`] and the sender's resolved
	/// power level (derived during resolution and supplied by the caller).
	#[must_use]
	pub fn from_view<V: EventView>(view: &V, power_level: i64) -> Self {
		let mut state = Self::new(&view.event_id(), &view.event_type(), &view.sender())
			.with_origin_server_ts(view.origin_server_ts())
			.with_power_level(power_level)
			.with_content(&view.content());

		if let Some(state_key) = view.state_key() {
			state = state.with_state_key(&state_key);
		}

		let auth = view.auth_event_ids();
		let auth_refs: Vec<&str> = auth.iter().map(String::as_str).collect();
		state.with_auth_events(&auth_refs)
	}
}

/// Extract an auth event id from an `auth_events` entry, accepting the v3+ form
/// (an event-id string) and the v1/v2 form (`[event_id, hashes]`).
fn auth_event_id(entry: &Value) -> Option<&str> {
	match entry {
		| Value::String(id) => Some(id),
		| Value::Array(pair) => pair.first().and_then(Value::as_str),
		| _ => None,
	}
}

impl Event for StateEvent {
	fn event_id(&self) -> &str { &self.event_id }

	fn event_type(&self) -> &str { &self.event_type }

	fn state_key(&self) -> &str { &self.state_key }

	fn power_level(&self) -> i64 { self.power_level }

	fn origin_server_ts(&self) -> u64 { self.origin_server_ts }

	fn auth_event_ids(&self) -> &[EventId] { &self.auth_event_ids }

	fn sender(&self) -> &str { &self.sender }

	fn power_levels(&self) -> Option<&PowerLevels> { self.power_levels.as_ref() }

	fn membership(&self) -> Option<&str> { self.membership.as_deref() }

	fn join_rule(&self) -> Option<&str> { self.join_rule.as_deref() }

	fn join_authorised_via_users_server(&self) -> Option<&str> {
		self.join_authorised.as_deref()
	}
}
