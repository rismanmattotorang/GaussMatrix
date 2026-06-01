//! Tests for capability scoping, mediation, and the agent event model.

use serde_json::json;

use crate::{
	Action, CapabilityGrant, Decision, DenyReason, Gateway, TOOL_CALL_TYPE, TOOL_RESULT_TYPE,
	ToolCall, ToolResult, mediation_record, tool_call_from_mcp, tool_result_to_mcp,
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
