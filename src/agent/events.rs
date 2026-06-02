//! The in-band agent event model (§IV-B).
//!
//! An agent's tool invocations and their results are reflected into the room as
//! structured, namespaced events so the interaction is visible, replayable, and
//! auditable in-band — there is no out-of-band side effect.

use serde_json::{Map, Value};

/// The event type of an agent tool invocation.
pub const TOOL_CALL_TYPE: &str = "m.gauss.agent.tool_call";

/// The event type of an agent tool result.
pub const TOOL_RESULT_TYPE: &str = "m.gauss.agent.tool_result";

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
