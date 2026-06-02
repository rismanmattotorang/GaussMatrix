//! Tests for capability scoping, mediation, and the agent event model.

use serde_json::json;

use crate::{
	Action, AgentProfile, CAPABILITY_GRANT_TYPE, CapabilityGrant, DEFAULT_AGENT_NAMESPACE, Decision,
	DenyReason, Gateway, ProvisionError, TOOL_APPROVAL_TYPE, TOOL_CALL_TYPE, TOOL_RESULT_TYPE,
	ToolApproval, ToolCall, ToolResult, handle_mcp, is_agent_id, mcp_call_ack, mediation_record,
	tool_call_from_mcp, tool_result_to_mcp,
};

fn grant() -> CapabilityGrant {
	CapabilityGrant::new()
		.allow_room("!room:example.org")
		.allow_tool("read_messages", Action::Auto)
		.allow_tool("invite_user", Action::Review)
		.allow_tool("modify_power_levels", Action::Forbidden)
}

#[test]
fn empty_grant_denies_everything() {
	let grant = CapabilityGrant::new();
	assert_eq!(
		grant.mediate("read_messages", "!room:example.org"),
		Decision::Denied(DenyReason::RoomNotInScope)
	);
}

#[test]
fn room_must_be_in_scope() {
	assert_eq!(
		grant().mediate("read_messages", "!other:example.org"),
		Decision::Denied(DenyReason::RoomNotInScope)
	);
}

#[test]
fn tool_must_be_granted() {
	assert_eq!(
		grant().mediate("delete_room", "!room:example.org"),
		Decision::Denied(DenyReason::ToolNotGranted)
	);
}

#[test]
fn classification_drives_the_decision() {
	let grant = grant();
	let room = "!room:example.org";
	assert_eq!(grant.mediate("read_messages", room), Decision::Execute);
	assert_eq!(grant.mediate("invite_user", room), Decision::RequiresApproval);
	assert_eq!(
		grant.mediate("modify_power_levels", room),
		Decision::Denied(DenyReason::ToolForbidden)
	);
}

#[test]
fn permitted_but_unclassified_tool_requires_approval() {
	// A grant whose default action governs a permitted tool with no explicit
	// classification (constructed via the conservative default).
	let grant = CapabilityGrant::new().allow_room("!r:x").allow_tool("send", Action::default());
	assert_eq!(grant.mediate("send", "!r:x"), Decision::RequiresApproval);
}

#[test]
fn mediation_record_is_audit_ready_json() {
	let decision = grant().mediate("invite_user", "!room:example.org");
	let record = mediation_record("@agent:example.org", "invite_user", "!room:example.org", decision);
	let parsed: serde_json::Value = serde_json::from_slice(&record).unwrap();
	assert_eq!(parsed["agent"], "@agent:example.org");
	assert_eq!(parsed["tool"], "invite_user");
	assert_eq!(parsed["room"], "!room:example.org");
	assert_eq!(parsed["decision"], "requires_approval");
}

#[test]
fn mediation_record_labels_denials() {
	let decision = grant().mediate("modify_power_levels", "!room:example.org");
	let record = mediation_record("@a:x", "modify_power_levels", "!room:example.org", decision);
	let parsed: serde_json::Value = serde_json::from_slice(&record).unwrap();
	assert_eq!(parsed["decision"], "denied:tool_forbidden");
}

#[test]
fn tool_call_event_content() {
	assert_eq!(TOOL_CALL_TYPE, "m.gauss.agent.tool_call");
	let call = ToolCall::new("c1", "read_messages", json!({ "room": "!r:x", "limit": 10 }));
	assert_eq!(
		call.to_content(),
		json!({
			"call_id": "c1",
			"tool": "read_messages",
			"arguments": { "room": "!r:x", "limit": 10 }
		})
	);
}

#[test]
fn tool_result_event_content() {
	assert_eq!(TOOL_RESULT_TYPE, "m.gauss.agent.tool_result");

	let ok = ToolResult::success("c1", json!({ "messages": 3 }));
	assert_eq!(ok.to_content(), json!({ "call_id": "c1", "output": { "messages": 3 } }));

	let err = ToolResult::failure("c2", "tool unavailable");
	assert_eq!(err.to_content(), json!({ "call_id": "c2", "error": "tool unavailable" }));
}

#[test]
fn gateway_executes_auto_tool_and_emits_event() {
	let gateway = Gateway::new("@agent:example.org", grant());
	let call = ToolCall::new("c1", "read_messages", json!({ "limit": 5 }));
	let mediation = gateway.process(&call, "!room:example.org");

	assert_eq!(mediation.decision, Decision::Execute);
	assert!(mediation.event.is_some(), "an executed call is reflected in-band");
	let parsed: serde_json::Value = serde_json::from_slice(&mediation.audit_record).unwrap();
	assert_eq!(parsed["decision"], "execute");
}

#[test]
fn gateway_denies_out_of_scope_and_suppresses_event() {
	let gateway = Gateway::new("@agent:example.org", grant());
	let call = ToolCall::new("c2", "read_messages", json!({}));
	let mediation = gateway.process(&call, "!elsewhere:example.org");

	assert_eq!(mediation.decision, Decision::Denied(DenyReason::RoomNotInScope));
	assert!(mediation.event.is_none(), "a denied call produces no in-band event");
	// The denial is still audited.
	let parsed: serde_json::Value = serde_json::from_slice(&mediation.audit_record).unwrap();
	assert_eq!(parsed["decision"], "denied:room_not_in_scope");
}

#[test]
fn gateway_review_tool_emits_event_pending_approval() {
	let gateway = Gateway::new("@agent:example.org", grant());
	let call = ToolCall::new("c3", "invite_user", json!({ "user": "@bob:example.org" }));
	let mediation = gateway.process(&call, "!room:example.org");
	assert_eq!(mediation.decision, Decision::RequiresApproval);
	assert!(mediation.event.is_some());
}

#[test]
fn mcp_tool_call_is_parsed() {
	let request = json!({
		"jsonrpc": "2.0",
		"id": 7,
		"method": "tools/call",
		"params": { "name": "read_messages", "arguments": { "limit": 3 } }
	});
	let call = tool_call_from_mcp(&request).unwrap();
	assert_eq!(call.call_id, "7");
	assert_eq!(call.tool, "read_messages");
	assert_eq!(call.arguments, json!({ "limit": 3 }));

	// Non-tools/call requests are not parsed.
	assert!(tool_call_from_mcp(&json!({ "method": "tools/list" })).is_none());
}

#[test]
fn mcp_tool_result_round_trips() {
	let ok = tool_result_to_mcp(&ToolResult::success("7", json!({ "n": 3 })));
	assert_eq!(ok["id"], "7");
	assert_eq!(ok["result"]["isError"], false);

	let err = tool_result_to_mcp(&ToolResult::failure("8", "boom"));
	assert_eq!(err["result"]["isError"], true);
	assert_eq!(err["result"]["content"][0]["text"], "boom");
}

#[test]
fn tool_result_parses_from_content() {
	// A success result round-trips through its event content.
	let success = ToolResult::success("7", json!({ "n": 3 }));
	assert_eq!(ToolResult::from_content(&success.to_content()), Some(success));

	// A failure result round-trips too.
	let failure = ToolResult::failure("8", "boom");
	assert_eq!(ToolResult::from_content(&failure.to_content()), Some(failure));

	// A null output is treated as absent, not as a successful empty output.
	let nulled = ToolResult::from_content(&json!({ "call_id": "9", "output": null }));
	assert_eq!(nulled, Some(ToolResult { call_id: "9".to_owned(), output: None, error: None }));

	// Missing call_id is rejected.
	assert!(ToolResult::from_content(&json!({ "output": 1 })).is_none());
}

#[test]
fn mcp_call_ack_reports_mediation_outcome() {
	let call = ToolCall::new("7", "read_messages", json!({}));

	// An accepted call: JSON-RPC result carrying the decision status.
	let ack = mcp_call_ack(&call, Decision::Execute);
	assert_eq!(ack["id"], "7");
	assert_eq!(ack["result"]["status"], "execute");
	assert_eq!(ack["result"]["isError"], false);
	assert!(ack.get("error").is_none());

	// Pending approval is likewise an accepted (non-error) result.
	let pending = mcp_call_ack(&call, Decision::RequiresApproval);
	assert_eq!(pending["result"]["status"], "requires_approval");

	// A denial: JSON-RPC error whose message is the decision label.
	let denied = mcp_call_ack(&call, Decision::Denied(DenyReason::ToolForbidden));
	assert!(denied.get("result").is_none());
	assert_eq!(denied["error"]["code"], -32_004);
	assert_eq!(denied["error"]["message"], "denied:tool_forbidden");
}

#[test]
fn tool_approval_round_trips_and_audits() {
	assert_eq!(TOOL_APPROVAL_TYPE, "m.gauss.agent.tool_approval");

	// An approval round-trips through its event content.
	let approved = ToolApproval::new("7", true, "@alice:example.org", Some("looks safe"));
	let restored = ToolApproval::from_content(&approved.to_content());
	assert_eq!(restored, Some(approved.clone()));
	assert_eq!(approved.to_content()["approved"], true);

	// A rejection without a reason round-trips too.
	let rejected = ToolApproval::new("8", false, "@bob:example.org", None);
	assert_eq!(ToolApproval::from_content(&rejected.to_content()), Some(rejected.clone()));

	// The audit record names the reviewer, the call, and the outcome.
	let record: serde_json::Value = serde_json::from_slice(&approved.audit_record()).unwrap();
	assert_eq!(record["reviewer"], "@alice:example.org");
	assert_eq!(record["call_id"], "7");
	assert_eq!(record["decision"], "approved");
	let rejected_record: serde_json::Value =
		serde_json::from_slice(&rejected.audit_record()).unwrap();
	assert_eq!(rejected_record["decision"], "rejected");

	// Missing required fields are rejected.
	assert!(ToolApproval::from_content(&json!({ "call_id": "9" })).is_none());
}

#[test]
fn agent_provisioning_validates_and_round_trips() {
	let agent = "@gauss.agent.scheduler:example.org";

	// A valid provisioning record round-trips through its stored content.
	let profile =
		AgentProfile::provision(agent, DEFAULT_AGENT_NAMESPACE, "scheduler-bot", "ed25519:AbC", Some("Scheduler"))
			.unwrap();
	assert_eq!(profile.agent_id, agent);
	assert_eq!(profile.operator, "scheduler-bot");
	assert_eq!(AgentProfile::from_content(&profile.to_content()), Some(profile));

	// A non-namespace id is rejected.
	assert_eq!(
		AgentProfile::provision("@alice:example.org", DEFAULT_AGENT_NAMESPACE, "op", "k", None),
		Err(ProvisionError::NotInNamespace),
	);

	// A missing signing key is rejected — agents must be cross-signed.
	assert_eq!(
		AgentProfile::provision(agent, DEFAULT_AGENT_NAMESPACE, "op", "  ", None),
		Err(ProvisionError::MissingSigningKey),
	);

	// Required fields are enforced on parse.
	assert!(AgentProfile::from_content(&json!({ "agent_id": agent })).is_none());
}

#[test]
fn capability_grant_round_trips_through_room_state_content() {
	assert_eq!(CAPABILITY_GRANT_TYPE, "m.gauss.agent.capability");

	let original = grant();
	let content = original.to_content();
	let restored = CapabilityGrant::from_content(&content);

	// Mediation behaves identically after a state round-trip.
	let room = "!room:example.org";
	for tool in ["read_messages", "invite_user", "modify_power_levels", "unknown"] {
		assert_eq!(
			original.mediate(tool, room),
			restored.mediate(tool, room),
			"decision differs for {tool} after round-trip"
		);
	}
	// And out-of-scope rooms remain denied.
	assert_eq!(
		restored.mediate("read_messages", "!elsewhere:example.org"),
		Decision::Denied(DenyReason::RoomNotInScope)
	);
}

#[test]
fn capability_grant_version_round_trips() {
	// A fresh grant is version 0.
	assert_eq!(CapabilityGrant::new().version(), 0);

	// The version is carried through the state-content round-trip.
	let grant = CapabilityGrant::new()
		.with_version(7)
		.with_default_action(Action::Forbidden)
		.allow_room("!room:example.org")
		.allow_tool("read_messages", Action::Auto);
	let restored = CapabilityGrant::from_content(&grant.to_content());

	assert_eq!(restored.version(), 7);
	assert_eq!(grant.to_content()["version"], 7);
	// The default action also survives, governing unclassified parse paths.
	assert_eq!(restored.mediate("read_messages", "!room:example.org"), Decision::Execute);

	// A grant with no version field defaults to 0.
	let legacy = CapabilityGrant::from_content(&json!({ "rooms": [], "tools": {} }));
	assert_eq!(legacy.version(), 0);
}

#[test]
fn capability_rate_limit_round_trips() {
	let grant = CapabilityGrant::new()
		.allow_room("!room:example.org")
		.allow_tool("send_message", Action::Auto)
		.with_rate_limit("send_message", 5, 60);

	// The limit is exposed and survives the state-content round-trip.
	let limit = grant.rate_limit_for("send_message").unwrap();
	assert_eq!(limit.max, 5);
	assert_eq!(limit.window_secs, 60);

	let restored = CapabilityGrant::from_content(&grant.to_content());
	assert_eq!(restored.rate_limit_for("send_message"), Some(limit));

	// The configured limits are enumerable.
	let listed: Vec<(&str, _)> = restored.rate_limits().collect();
	assert_eq!(listed, vec![("send_message", limit)]);

	// Tools without a configured limit report none.
	assert!(grant.rate_limit_for("other").is_none());

	// The rate-limited deny reason has a stable label.
	assert_eq!(
		Decision::Denied(DenyReason::RateLimited).label(),
		"denied:rate_limited"
	);
}

#[test]
fn capability_content_has_expected_shape() {
	let content = grant().to_content();
	assert_eq!(content["tools"]["read_messages"], "auto");
	assert_eq!(content["tools"]["invite_user"], "review");
	assert_eq!(content["tools"]["modify_power_levels"], "forbidden");
	assert_eq!(content["default_action"], "review");
	assert!(content["rooms"].as_array().unwrap().iter().any(|r| r == "!room:example.org"));
}

#[test]
fn agent_namespace_membership() {
	assert!(is_agent_id("@gauss.agent.summariser:example.org", DEFAULT_AGENT_NAMESPACE));
	assert!(!is_agent_id("@alice:example.org", DEFAULT_AGENT_NAMESPACE));
}

#[test]
fn mcp_tools_list_is_scoped_to_callable_tools() {
	let response = handle_mcp(&grant(), &json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))
		.unwrap();
	let names: Vec<&str> = response["result"]["tools"]
		.as_array()
		.unwrap()
		.iter()
		.map(|t| t["name"].as_str().unwrap())
		.collect();

	assert!(names.contains(&"read_messages"));
	assert!(names.contains(&"invite_user"));
	// A forbidden tool is never offered.
	assert!(!names.contains(&"modify_power_levels"));
	assert_eq!(response["id"], 1);
}

#[test]
fn mcp_resources_list_exposes_accessible_rooms() {
	let response =
		handle_mcp(&grant(), &json!({ "id": "a", "method": "resources/list" })).unwrap();
	let resources = response["result"]["resources"].as_array().unwrap();
	assert_eq!(resources.len(), 1);
	assert_eq!(resources[0]["uri"], "matrix://room/!room:example.org");
}

#[test]
fn mcp_dispatcher_ignores_tools_call_and_unknown_methods() {
	// tools/call proceeds through the gateway, not the read-only dispatcher.
	assert!(handle_mcp(&grant(), &json!({ "method": "tools/call" })).is_none());
	assert!(handle_mcp(&grant(), &json!({ "method": "prompts/list" })).is_none());
}
