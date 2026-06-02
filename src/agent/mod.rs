//! # gm-agent — GaussMatrix agentic layer (policy core)
//!
//! The agentic surface is the specification's most distinctive contribution
//! ([`GaussMatrix-SPECS.pdf`], §IV): AI agents are governed Matrix principals
//! reached through a gateway that is the *sole* channel by which an agent acts.
//! Its guiding invariant is that admitting an agent to a room never enlarges
//! that room's trust boundary beyond the humans who admitted it — every agent
//! action must be authenticated, **scoped**, **mediated**, and **auditable**.
//!
//! This crate provides the storage- and transport-pure policy core of that
//! gateway:
//!
//! * [`CapabilityGrant`] — a least-privilege grant of permitted tools,
//!   accessible rooms, and a per-tool `auto`/`review`/`forbidden` classification
//!   (§IV-C). [`CapabilityGrant::mediate`] turns a tool invocation into a
//!   [`Decision`]: execute immediately, require human approval, or deny.
//! * [`ToolCall`] / [`ToolResult`] — the in-band, namespaced agent events
//!   (`m.gauss.agent.tool_call` / `m.gauss.agent.tool_result`, §IV-B) that make
//!   an interaction visible, replayable, and auditable.
//! * [`mediation_record`] — a serialised record of a gateway decision, suitable
//!   for appending to the tamper-evident audit log (§IV-D).
//!
//! The Model-Context-Protocol transport, cross-signed agent provisioning, and
//! the E2EE-aware mediation that wire this into the live server build on this
//! policy core.
//!
//! [`GaussMatrix-SPECS.pdf`]: ../../GaussMatrix-SPECS.pdf

#![forbid(unsafe_code)]

mod capability;
mod events;
mod gateway;
mod mcp;
mod provisioning;
#[cfg(test)]
mod tests;

pub use self::{
	capability::{
		Action, CAPABILITY_GRANT_TYPE, CapabilityGrant, Decision, DenyReason, RateLimit,
		mediation_record,
	},
	events::{
		TOOL_APPROVAL_TYPE, TOOL_CALL_TYPE, TOOL_RESULT_TYPE, ToolApproval, ToolCall, ToolResult,
	},
	gateway::{Gateway, Mediation},
	mcp::{handle_mcp, mcp_call_ack, tool_call_from_mcp, tool_result_to_mcp},
	provisioning::{AgentProfile, DEFAULT_AGENT_NAMESPACE, ProvisionError, is_agent_id},
};
