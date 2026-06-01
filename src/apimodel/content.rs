//! Parsers from Matrix event-content JSON into the resolution models.

use std::collections::BTreeMap;

use gm_stateres::PowerLevels;
use serde_json::Value;

/// Matrix content defaults for a present `m.room.power_levels` event.
const DEFAULT_STATE: i64 = 50;
const DEFAULT_BAN: i64 = 50;
const DEFAULT_KICK: i64 = 50;

/// Parse `m.room.power_levels` content into [`PowerLevels`].
///
/// Omitted fields take the Matrix defaults (`state_default`/`ban`/`kick` = 50;
/// `users_default`/`events_default`/`invite` = 0). Power-level values are
/// accepted as integers or as the integer-encoded strings older rooms emit.
#[must_use]
pub fn power_levels_from_content(content: &Value) -> PowerLevels {
	PowerLevels {
		users: parse_level_map(content.get("users")),
		users_default: parse_level(content.get("users_default")).unwrap_or(0),
		events: parse_level_map(content.get("events")),
		events_default: parse_level(content.get("events_default")).unwrap_or(0),
		state_default: parse_level(content.get("state_default")).unwrap_or(DEFAULT_STATE),
		invite: parse_level(content.get("invite")).unwrap_or(0),
		kick: parse_level(content.get("kick")).unwrap_or(DEFAULT_KICK),
		ban: parse_level(content.get("ban")).unwrap_or(DEFAULT_BAN),
	}
}

/// Extract the `membership` from `m.room.member` content.
#[must_use]
pub fn membership_from_content(content: &Value) -> Option<String> {
	content
		.get("membership")
		.and_then(Value::as_str)
		.map(ToOwned::to_owned)
}

/// Extract the `join_rule` from `m.room.join_rules` content.
#[must_use]
pub fn join_rule_from_content(content: &Value) -> Option<String> {
	content
		.get("join_rule")
		.and_then(Value::as_str)
		.map(ToOwned::to_owned)
}

/// Extract `join_authorised_via_users_server` from `m.room.member` content (set
/// on restricted-room joins).
#[must_use]
pub fn join_authorised_from_content(content: &Value) -> Option<String> {
	content
		.get("join_authorised_via_users_server")
		.and_then(Value::as_str)
		.map(ToOwned::to_owned)
}

/// Parse a single power-level value, accepting an integer or an integer-encoded
/// string.
fn parse_level(value: Option<&Value>) -> Option<i64> {
	match value? {
		| Value::Number(number) => number.as_i64(),
		| Value::String(string) => string.parse().ok(),
		| _ => None,
	}
}

/// Parse a `{ key: level }` object, dropping entries whose value is not a valid
/// power level.
fn parse_level_map(value: Option<&Value>) -> BTreeMap<String, i64> {
	value
		.and_then(Value::as_object)
		.map(|object| {
			object
				.iter()
				.filter_map(|(key, val)| parse_level(Some(val)).map(|level| (key.clone(), level)))
				.collect()
		})
		.unwrap_or_default()
}
