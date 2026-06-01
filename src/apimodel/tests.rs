//! Tests for the content parsers and the `StateEvent` adapter.

use std::collections::BTreeMap;

use gm_stateres::{AllOf, AuthRules, CreateRules, Event, MembershipRules, PowerLevelRules, StateMap};
use serde_json::json;

use crate::{
	AuthScope, Endpoint, ErrorCode, MatrixError, Method, Route, Router, StateEvent, Versions,
	extract_access_token, join_rule_from_content, match_template, membership_from_content,
	power_levels_from_content,
};

#[test]
fn power_levels_apply_matrix_defaults_for_omitted_fields() {
	let pl = power_levels_from_content(&json!({ "users": { "@a:x": 100 } }));
	assert_eq!(pl.for_user("@a:x"), 100);
	assert_eq!(pl.for_user("@b:x"), 0); // users_default
	assert_eq!(pl.state_default, 50);
	assert_eq!(pl.ban, 50);
	assert_eq!(pl.kick, 50);
	assert_eq!(pl.invite, 0);
	assert_eq!(pl.events_default, 0);
}

#[test]
fn power_levels_accept_integer_or_string_values() {
	let pl = power_levels_from_content(&json!({
		"users": { "@a:x": "75" },
		"state_default": "60",
		"ban": 70
	}));
	assert_eq!(pl.for_user("@a:x"), 75); // string-encoded integer
	assert_eq!(pl.state_default, 60); // string-encoded integer
	assert_eq!(pl.ban, 70); // integer
}

#[test]
fn power_levels_required_for_uses_explicit_and_default() {
	let pl = power_levels_from_content(&json!({ "events": { "m.room.name": 80 } }));
	assert_eq!(pl.required_for("m.room.name", true), 80); // explicit
	assert_eq!(pl.required_for("m.room.topic", true), 50); // state_default
}

#[test]
fn membership_and_join_rule_extraction() {
	assert_eq!(membership_from_content(&json!({ "membership": "join" })).as_deref(), Some("join"));
	assert_eq!(membership_from_content(&json!({})).as_deref(), None);
	assert_eq!(join_rule_from_content(&json!({ "join_rule": "public" })).as_deref(), Some("public"));
	assert_eq!(join_rule_from_content(&json!({})).as_deref(), None);
}

#[test]
fn state_event_projects_wire_fields() {
	let event = StateEvent::new("$e", "m.room.name", "@alice")
		.with_state_key("")
		.with_origin_server_ts(42)
		.with_auth_events(&["$c", "$pl"])
		.with_power_level(50);

	assert_eq!(event.event_id(), "$e");
	assert_eq!(event.event_type(), "m.room.name");
	assert_eq!(event.sender(), "@alice");
	assert_eq!(event.origin_server_ts(), 42);
	assert_eq!(event.power_level(), 50);
	assert_eq!(event.auth_event_ids(), &["$c".to_owned(), "$pl".to_owned()]);
}

#[test]
fn state_event_parses_typed_content() {
	let pl = StateEvent::new("$pl", "m.room.power_levels", "@creator")
		.with_content(&json!({ "users": { "@creator": 100 } }));
	assert_eq!(pl.power_levels().unwrap().for_user("@creator"), 100);

	let member = StateEvent::new("$m", "m.room.member", "@alice")
		.with_state_key("@alice")
		.with_content(&json!({ "membership": "join" }));
	assert_eq!(member.membership(), Some("join"));

	let rules = StateEvent::new("$jr", "m.room.join_rules", "@creator")
		.with_content(&json!({ "join_rule": "public" }));
	assert_eq!(rules.join_rule(), Some("public"));

	// Content on an unrelated type leaves the projections empty.
	let name = StateEvent::new("$n", "m.room.name", "@alice")
		.with_content(&json!({ "name": "Room" }));
	assert!(name.power_levels().is_none() && name.membership().is_none());
}

#[test]
fn state_events_drive_the_resolution_rules() {
	// A power-levels event and two topic events parsed from wire content,
	// resolved against the gm-stateres rules.
	let pl_content = json!({ "users": { "@creator": 100 }, "state_default": 50 });
	let store: BTreeMap<String, StateEvent> = [
		StateEvent::new("$c", "m.room.create", "@creator"),
		StateEvent::new("$pl", "m.room.power_levels", "@creator").with_content(&pl_content),
		StateEvent::new("$ok", "m.room.topic", "@creator").with_auth_events(&["$c", "$pl"]),
		StateEvent::new("$no", "m.room.topic", "@bob").with_auth_events(&["$c", "$pl"]),
	]
	.into_iter()
	.map(|event| (event.event_id().to_owned(), event))
	.collect();

	let mut state = StateMap::new();
	state.insert(("m.room.create".to_owned(), String::new()), "$c".to_owned());
	state.insert(("m.room.power_levels".to_owned(), String::new()), "$pl".to_owned());

	let rules = PowerLevelRules;
	// @creator (100 >= 50) authorised; @bob (default 0) rejected.
	assert!(rules.is_authorized(&store["$ok"], &state, &store));
	assert!(!rules.is_authorized(&store["$no"], &state, &store));

	// Composed with create and membership gates via AllOf.
	let components: [&dyn AuthRules<StateEvent>; 3] =
		[&CreateRules, &PowerLevelRules, &MembershipRules];
	let all = AllOf(&components);
	assert!(all.is_authorized(&store["$ok"], &state, &store));
}

#[test]
fn from_event_json_parses_a_v3_member_event() {
	let event = json!({
		"type": "m.room.member",
		"sender": "@alice:example.org",
		"state_key": "@alice:example.org",
		"origin_server_ts": 1000,
		"auth_events": ["$c", "$pl"],
		"content": { "membership": "join" }
	});
	let parsed = StateEvent::from_event_json("$evt", &event).unwrap();

	assert_eq!(parsed.event_id(), "$evt");
	assert_eq!(parsed.event_type(), "m.room.member");
	assert_eq!(parsed.sender(), "@alice:example.org");
	assert_eq!(parsed.state_key(), "@alice:example.org");
	assert_eq!(parsed.origin_server_ts(), 1000);
	assert_eq!(parsed.auth_event_ids(), &["$c".to_owned(), "$pl".to_owned()]);
	assert_eq!(parsed.membership(), Some("join"));
}

#[test]
fn from_event_json_accepts_v1_auth_event_pairs() {
	// Room v1/v2 carry auth_events as [event_id, hashes] pairs.
	let event = json!({
		"type": "m.room.power_levels",
		"sender": "@a:x",
		"state_key": "",
		"origin_server_ts": 5,
		"auth_events": [["$c", { "sha256": "abc" }], ["$prev", { "sha256": "def" }]],
		"content": { "users": { "@a:x": 100 } }
	});
	let parsed = StateEvent::from_event_json("$pl", &event).unwrap();
	assert_eq!(parsed.auth_event_ids(), &["$c".to_owned(), "$prev".to_owned()]);
	assert_eq!(parsed.power_levels().unwrap().for_user("@a:x"), 100);
}

#[test]
fn from_event_json_parses_restricted_join_authoriser() {
	let event = json!({
		"type": "m.room.member",
		"sender": "@alice:x",
		"state_key": "@alice:x",
		"origin_server_ts": 1,
		"content": { "membership": "join", "join_authorised_via_users_server": "@admin:x" }
	});
	let parsed = StateEvent::from_event_json("$j", &event).unwrap();
	assert_eq!(parsed.membership(), Some("join"));
	assert_eq!(parsed.join_authorised_via_users_server(), Some("@admin:x"));
}

#[test]
fn from_event_json_rejects_events_missing_required_fields() {
	// No sender / origin_server_ts.
	assert!(StateEvent::from_event_json("$x", &json!({ "type": "m.room.name" })).is_none());
	// No type.
	assert!(
		StateEvent::from_event_json("$x", &json!({ "sender": "@a:x", "origin_server_ts": 1 }))
			.is_none()
	);
}

#[test]
fn error_codes_map_to_errcode_and_status() {
	assert_eq!(ErrorCode::Forbidden.errcode(), "M_FORBIDDEN");
	assert_eq!(ErrorCode::Forbidden.http_status(), 403);
	assert_eq!(ErrorCode::UnknownToken.http_status(), 401);
	assert_eq!(ErrorCode::NotFound.http_status(), 404);
	assert_eq!(ErrorCode::Unrecognized.http_status(), 404);
	assert_eq!(ErrorCode::LimitExceeded.errcode(), "M_LIMIT_EXCEEDED");
	assert_eq!(ErrorCode::LimitExceeded.http_status(), 429);
	assert_eq!(ErrorCode::TooLarge.http_status(), 413);
	assert_eq!(ErrorCode::BadJson.http_status(), 400);
	assert_eq!(ErrorCode::Unknown.http_status(), 500);
}

#[test]
fn matrix_error_serializes_to_wire_body() {
	let err = MatrixError::new(ErrorCode::Forbidden, "nope");
	assert_eq!(err.http_status(), 403);
	assert_eq!(err.to_json(), json!({ "errcode": "M_FORBIDDEN", "error": "nope" }));
}

#[test]
fn rate_limited_error_includes_retry_after_and_displays() {
	let err = MatrixError::rate_limited("slow down", 2000);
	assert_eq!(err.http_status(), 429);
	assert_eq!(
		err.to_json(),
		json!({ "errcode": "M_LIMIT_EXCEEDED", "error": "slow down", "retry_after_ms": 2000 })
	);
	assert_eq!(err.to_string(), "M_LIMIT_EXCEEDED: slow down");
}

#[test]
fn match_template_captures_path_parameters() {
	let params = match_template(
		"/_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}",
		"/_matrix/client/v3/rooms/!r:x/send/m.room.message/123",
	)
	.unwrap();
	assert_eq!(params["roomId"], "!r:x");
	assert_eq!(params["eventType"], "m.room.message");
	assert_eq!(params["txnId"], "123");
}

#[test]
fn match_template_rejects_literal_and_arity_mismatches() {
	// Different literal segment.
	assert!(match_template("/_matrix/client/versions", "/_matrix/client/v3").is_none());
	// Too few / too many segments.
	assert!(match_template("/a/{b}/c", "/a/x").is_none());
	assert!(match_template("/a/{b}", "/a/x/y").is_none());
	// Exact literal match with no params.
	assert_eq!(match_template("/_matrix/client/versions", "/_matrix/client/versions").unwrap().len(), 0);
}

#[test]
fn endpoint_matches_method_and_path() {
	let ep = Endpoint::new(Method::Get, "/_matrix/client/v3/rooms/{roomId}/state", AuthScope::User);
	assert_eq!(ep.auth, AuthScope::User);

	let params = ep.matches(Method::Get, "/_matrix/client/v3/rooms/!r:x/state").unwrap();
	assert_eq!(params["roomId"], "!r:x");

	// Wrong method → no match.
	assert!(ep.matches(Method::Post, "/_matrix/client/v3/rooms/!r:x/state").is_none());
	// Wrong path → no match.
	assert!(ep.matches(Method::Get, "/_matrix/client/versions").is_none());
}

#[test]
fn access_token_prefers_bearer_header() {
	assert_eq!(
		extract_access_token(Some("Bearer secret123"), Some("qtok")).as_deref(),
		Some("secret123")
	);
	// Header present but not Bearer → no token, no query fallback.
	assert_eq!(extract_access_token(Some("Basic abc"), Some("qtok")), None);
	// Empty bearer token → none.
	assert_eq!(extract_access_token(Some("Bearer "), None), None);
}

#[test]
fn access_token_falls_back_to_query_param() {
	assert_eq!(extract_access_token(None, Some("qtok")).as_deref(), Some("qtok"));
	assert_eq!(extract_access_token(None, None), None);
	assert_eq!(extract_access_token(None, Some("")), None);
}

#[test]
fn versions_response_serializes() {
	let versions = Versions::new(&["v1.11", "v1.12"])
		.with_unstable_feature("org.matrix.msc3575", true);
	assert_eq!(
		versions.to_json(),
		json!({
			"versions": ["v1.11", "v1.12"],
			"unstable_features": { "org.matrix.msc3575": true }
		})
	);
}

fn router() -> Router {
	let mut router = Router::new();
	router.register(Endpoint::new(Method::Get, "/_matrix/client/versions", AuthScope::None));
	router.register(Endpoint::new(
		Method::Get,
		"/_matrix/client/v3/rooms/{roomId}/state",
		AuthScope::User,
	));
	router.register(Endpoint::new(
		Method::Put,
		"/_matrix/client/v3/rooms/{roomId}/state",
		AuthScope::User,
	));
	router
}

#[test]
fn router_resolves_a_matching_endpoint_with_params() {
	let router = router();
	let route = router.resolve(Method::Get, "/_matrix/client/v3/rooms/!r:x/state");
	match route {
		| Route::Matched { endpoint, params } => {
			assert_eq!(endpoint.method, Method::Get);
			assert_eq!(endpoint.auth, AuthScope::User);
			assert_eq!(params["roomId"], "!r:x");
		},
		| other => panic!("expected a match, got {other:?}"),
	}
}

#[test]
fn router_distinguishes_method_not_allowed_from_not_found() {
	let router = router();
	// Known path, unsupported method → 405.
	assert_eq!(
		router.resolve(Method::Post, "/_matrix/client/v3/rooms/!r:x/state"),
		Route::MethodNotAllowed
	);
	// Unknown path → 404 / M_UNRECOGNIZED.
	assert_eq!(router.resolve(Method::Get, "/_matrix/client/v3/nope"), Route::NotFound);
}
