//! The Model Context Protocol bridge (§IV-B).
//!
//! Inbound, an agent's MCP `tools/call` (a JSON-RPC 2.0 request) is parsed into
//! a [`ToolCall`] for the gateway to mediate. Outbound, a [`ToolResult`] is
//! rendered as the MCP `tools/call` response. This is the wire translation
//! between the Model Context Protocol and the in-band agent event model.

use serde_json::{Value, json};

use crate::{
	capability::{Action, CapabilityGrant, Decision},
	events::{ToolCall, ToolResult},
};

/// JSON-RPC error code returned when a tool call is rejected by the capability
/// grant — the implementation-defined server-error range (JSON-RPC 2.0 §5.1).
const MCP_FORBIDDEN: i64 = -32_004;

/// Handle a read-only MCP request (`tools/list`, `resources/list`) against a
/// capability grant, returning the JSON-RPC response.
///
/// The listings are **scoped to the grant**: only callable tools (forbidden
/// ones are withheld) and only accessible rooms are exposed — the inbound half
/// of the gateway (§IV-B), which never reveals more than the agent was granted.
/// Returns `None` for other methods (`tools/call` proceeds through the
/// [`Gateway`](crate::Gateway), not here).
#[must_use]
pub fn handle_mcp(grant: &CapabilityGrant, request: &Value) -> Option<Value> {
	let method = request.get("method").and_then(Value::as_str)?;
	let result = match method {
		| "tools/list" => tools_list_result(grant),
		| "resources/list" => resources_list_result(grant),
		| _ => return None,
	};

	let id = request.get("id").cloned().unwrap_or(Value::Null);
	Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

/// The `tools/list` result: the grant's callable tools (forbidden ones omitted).
fn tools_list_result(grant: &CapabilityGrant) -> Value {
	let tools: Vec<Value> = grant
		.tools()
		.filter(|(_, action)| *action != Action::Forbidden)
		.map(|(name, action)| json!({ "name": name, "annotations": { "action": action.label() } }))
		.collect();

	json!({ "tools": tools })
}

/// The `resources/list` result: the grant's accessible rooms as MCP resources.
fn resources_list_result(grant: &CapabilityGrant) -> Value {
	let resources: Vec<Value> = grant
		.rooms()
		.map(|room| json!({ "uri": format!("matrix://room/{room}"), "name": room }))
		.collect();

	json!({ "resources": resources })
}

/// Parse an MCP `tools/call` JSON-RPC request into a [`ToolCall`].
///
/// Returns `None` unless the request's `method` is `tools/call` and its
/// `params.name` is present. The JSON-RPC `id` becomes the call's correlation
/// id; `params.arguments` defaults to null when absent.
#[must_use]
pub fn tool_call_from_mcp(request: &Value) -> Option<ToolCall> {
	if request.get("method").and_then(Value::as_str) != Some("tools/call") {
		return None;
	}

	let params = request.get("params")?;
	let tool = params.get("name").and_then(Value::as_str)?;
	let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);

	Some(ToolCall::new(&request_id(request.get("id")), tool, arguments))
}

/// Render a [`ToolResult`] as an MCP `tools/call` JSON-RPC response.
///
/// A tool-execution failure is reported as a successful JSON-RPC response whose
/// `result.isError` is `true` (per MCP), not as a JSON-RPC error.
#[must_use]
pub fn tool_result_to_mcp(result: &ToolResult) -> Value {
	let (text, is_error) = match (&result.output, &result.error) {
		| (Some(output), _) => (output.to_string(), false),
		| (None, Some(error)) => (error.clone(), true),
		| (None, None) => (String::new(), false),
	};

	json!({
		"jsonrpc": "2.0",
		"id": result.call_id,
		"result": {
			"content": [ { "type": "text", "text": text } ],
			"isError": is_error,
		},
	})
}

/// Render the synchronous MCP `tools/call` response for a *mediated* call.
///
/// GaussMatrix mediates and records the call in-band; the tool's actual result
/// returns later as an `m.gauss.agent.tool_result`. This acknowledges only the
/// mediation outcome: an accepted or pending-approval call yields a `result`
/// carrying the decision `status`, while a denial yields a JSON-RPC `error`
/// whose message is the decision label — so a caller can tell "rejected" from
/// "accepted, awaiting result" without inspecting the timeline.
#[must_use]
pub fn mcp_call_ack(call: &ToolCall, decision: Decision) -> Value {
	let id = Value::from(call.call_id.clone());

	if decision.is_denied() {
		return json!({
			"jsonrpc": "2.0",
			"id": id,
			"error": { "code": MCP_FORBIDDEN, "message": decision.label() },
		});
	}

	json!({
		"jsonrpc": "2.0",
		"id": id,
		"result": {
			"status": decision.label(),
			"content": [ {
				"type": "text",
				"text": format!("tool call '{}' {}", call.tool, decision.label()),
			} ],
			"isError": false,
		},
	})
}

/// The JSON-RPC id as a correlation string (numbers stringified, missing → "").
fn request_id(id: Option<&Value>) -> String {
	match id {
		| Some(Value::String(string)) => string.clone(),
		| Some(Value::Number(number)) => number.to_string(),
		| _ => String::new(),
	}
}
