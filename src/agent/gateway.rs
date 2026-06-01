//! The mediation gateway — the sole channel through which an agent acts.
//!
//! The gateway ties the policy pieces into the flow of §IV (Fig. 3): a tool call
//! is checked against the agent's [`CapabilityGrant`], the decision is recorded
//! for the tamper-evident audit log, and — when the call proceeds — it is
//! reflected in-band as a namespaced event. There is no out-of-band side effect:
//! every action passes through here.

use serde_json::Value;

use crate::{
	capability::{CapabilityGrant, Decision, mediation_record},
	events::{ToolCall, ToolResult},
};

/// The result of the gateway mediating a tool call.
#[derive(Clone, Debug)]
pub struct Mediation {
	/// What the gateway decided.
	pub decision: Decision,
	/// The audit-log record of this decision (always produced, even on denial).
	pub audit_record: Vec<u8>,
	/// The in-band `m.gauss.agent.tool_call` event content to post when the call
	/// proceeds (executed or pending approval); `None` when denied.
	pub event: Option<Value>,
}

/// A per-agent mediation gateway.
#[derive(Clone, Debug)]
pub struct Gateway {
	agent: String,
	grant: CapabilityGrant,
}

impl Gateway {
	/// A gateway for `agent` governed by `grant`.
	#[must_use]
	pub fn new(agent: &str, grant: CapabilityGrant) -> Self {
		Self { agent: agent.to_owned(), grant }
	}

	/// The agent this gateway governs.
	#[must_use]
	pub fn agent(&self) -> &str { &self.agent }

	/// Mediate a tool call in `room`: decide, produce the audit record, and — if
	/// the call proceeds — the in-band tool-call event.
	#[must_use]
	pub fn process(&self, call: &ToolCall, room: &str) -> Mediation {
		let decision = self.grant.mediate(&call.tool, room);
		let audit_record = mediation_record(&self.agent, &call.tool, room, decision);
		let event = match decision {
			| Decision::Denied(_) => None,
			| Decision::Execute | Decision::RequiresApproval => Some(call.to_content()),
		};

		Mediation { decision, audit_record, event }
	}

	/// The in-band `m.gauss.agent.tool_result` event content for a completed
	/// call.
	#[must_use]
	pub fn result_event(&self, result: &ToolResult) -> Value { result.to_content() }
}
