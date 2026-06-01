//! The Model Context Protocol bridge (§IV-B).
//!
//! Inbound, an agent's MCP `tools/call` (a JSON-RPC 2.0 request) is parsed into
//! a [`ToolCall`] for the gateway to mediate. Outbound, a [`ToolResult`] is
//! rendered as the MCP `tools/call` response. This is the wire translation
//! between the Model Context Protocol and the in-band agent event model.

use serde_json::{Value, json};

use crate::events::{ToolCall, ToolResult};

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

/// The JSON-RPC id as a correlation string (numbers stringified, missing → "").
fn request_id(id: Option<&Value>) -> String {
	match id {
		| Some(Value::String(string)) => string.clone(),
		| Some(Value::Number(number)) => number.to_string(),
		| _ => String::new(),
	}
}
