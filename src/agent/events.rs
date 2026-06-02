//! The in-band agent event model (§IV-B).
//!
//! An agent's tool invocations and their results are reflected into the room as
//! structured, namespaced events so the interaction is visible, replayable, and
//! auditable in-band — there is no out-of-band side effect.

use serde_json::{Map, Value, json};

/// The event type of an agent tool invocation.
pub const TOOL_CALL_TYPE: &str = "m.gauss.agent.tool_call";

/// The event type of an agent tool result.
pub const TOOL_RESULT_TYPE: &str = "m.gauss.agent.tool_result";

/// The event type of a human-in-the-loop approval decision on a tool call.
pub const TOOL_APPROVAL_TYPE: &str = "m.gauss.agent.tool_approval";

/// An agent's invocation of a tool, recorded in-band.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCall {
	/// Correlates this call with its [`ToolResult`].
	pub call_id: String,
	/// The invoked tool's name.
	pub tool: String,
	/// The invocation arguments.
	pub arguments: Value,
}

impl ToolCall {
	/// A new tool call.
	#[must_use]
	pub fn new(call_id: &str, tool: &str, arguments: Value) -> Self {
		Self { call_id: call_id.to_owned(), tool: tool.to_owned(), arguments }
	}

	/// The `m.gauss.agent.tool_call` event content.
	#[must_use]
	pub fn to_content(&self) -> Value {
		let mut body = Map::new();
		body.insert("call_id".to_owned(), Value::from(self.call_id.clone()));
		body.insert("tool".to_owned(), Value::from(self.tool.clone()));
		body.insert("arguments".to_owned(), self.arguments.clone());

		Value::Object(body)
	}
}

/// The result of an agent tool invocation, recorded in-band.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolResult {
	/// The `call_id` of the [`ToolCall`] this answers.
	pub call_id: String,
	/// The successful output, if any.
	pub output: Option<Value>,
	/// The error message, if the invocation failed.
	pub error: Option<String>,
}

impl ToolResult {
	/// A successful result.
	#[must_use]
	pub fn success(call_id: &str, output: Value) -> Self {
		Self { call_id: call_id.to_owned(), output: Some(output), error: None }
	}

	/// A failed result.
	#[must_use]
	pub fn failure(call_id: &str, error: &str) -> Self {
		Self { call_id: call_id.to_owned(), output: None, error: Some(error.to_owned()) }
	}

	/// Parse a tool result from `m.gauss.agent.tool_result`-style content.
	///
	/// `call_id` is required (it correlates the result with its [`ToolCall`]);
	/// `output` and `error` are optional and a null `output` is treated as
	/// absent. The inverse of [`to_content`](Self::to_content). Returns `None`
	/// when no `call_id` is present.
	#[must_use]
	pub fn from_content(content: &Value) -> Option<Self> {
		let call_id = content.get("call_id").and_then(Value::as_str)?;
		let output = content.get("output").cloned().filter(|value| !value.is_null());
		let error = content.get("error").and_then(Value::as_str).map(ToOwned::to_owned);

		Some(Self { call_id: call_id.to_owned(), output, error })
	}

	/// The `m.gauss.agent.tool_result` event content.
	#[must_use]
	pub fn to_content(&self) -> Value {
		let mut body = Map::new();
		body.insert("call_id".to_owned(), Value::from(self.call_id.clone()));
		if let Some(output) = &self.output {
			body.insert("output".to_owned(), output.clone());
		}
		if let Some(error) = &self.error {
			body.insert("error".to_owned(), Value::from(error.clone()));
		}

		Value::Object(body)
	}
}

/// A human-in-the-loop decision on a tool call that required approval (§IV-C).
///
/// When a call mediates to `RequiresApproval`, a human reviewer approves or
/// rejects it; the decision is recorded in-band (correlated to the call by
/// `call_id`) and in the audit log, so the human gate is itself auditable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolApproval {
	/// The `call_id` of the [`ToolCall`] this decides.
	pub call_id: String,
	/// Whether the call was approved (`true`) or rejected (`false`).
	pub approved: bool,
	/// The reviewer who made the decision.
	pub reviewer: String,
	/// An optional human-readable rationale.
	pub reason: Option<String>,
}

impl ToolApproval {
	/// A new approval decision.
	#[must_use]
	pub fn new(call_id: &str, approved: bool, reviewer: &str, reason: Option<&str>) -> Self {
		Self {
			call_id: call_id.to_owned(),
			approved,
			reviewer: reviewer.to_owned(),
			reason: reason.map(ToOwned::to_owned),
		}
	}

	/// The `m.gauss.agent.tool_approval` event content.
	#[must_use]
	pub fn to_content(&self) -> Value {
		let mut body = Map::new();
		body.insert("call_id".to_owned(), Value::from(self.call_id.clone()));
		body.insert("approved".to_owned(), Value::from(self.approved));
		body.insert("reviewer".to_owned(), Value::from(self.reviewer.clone()));
		if let Some(reason) = &self.reason {
			body.insert("reason".to_owned(), Value::from(reason.clone()));
		}

		Value::Object(body)
	}

	/// Parse an approval decision from its event content; `call_id`, `approved`,
	/// and `reviewer` are required. The inverse of [`to_content`](Self::to_content).
	#[must_use]
	pub fn from_content(content: &Value) -> Option<Self> {
		let call_id = content.get("call_id").and_then(Value::as_str)?;
		let approved = content.get("approved").and_then(Value::as_bool)?;
		let reviewer = content.get("reviewer").and_then(Value::as_str)?;
		let reason = content.get("reason").and_then(Value::as_str).map(ToOwned::to_owned);

		Some(Self { call_id: call_id.to_owned(), approved, reviewer: reviewer.to_owned(), reason })
	}

	/// The audit-log record of this decision (§IV-D): the reviewer, the call, and
	/// the outcome.
	#[must_use]
	pub fn audit_record(&self) -> Vec<u8> {
		let body = json!({
			"reviewer": self.reviewer,
			"call_id": self.call_id,
			"decision": if self.approved { "approved" } else { "rejected" },
		});

		serde_json::to_vec(&body).unwrap_or_default()
	}
}
