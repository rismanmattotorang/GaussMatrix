//! Tests for capability scoping, mediation, and the agent event model.

use serde_json::json;

use crate::{
	Action, CapabilityGrant, Decision, DenyReason, TOOL_CALL_TYPE, TOOL_RESULT_TYPE, ToolCall,
	ToolResult, mediation_record,
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
