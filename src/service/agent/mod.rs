//! The agentic gateway service — the live mediation loop (SPECS §IV).
//!
//! This wires the storage- and transport-pure `gm-agent` policy core into the
//! running server. The live loop is: read the agent's [`CapabilityGrant`] from
//! the room's `m.gauss.agent.capability` state (§IV-C), mediate a tool call
//! through the [`Gateway`], record the decision in the live tamper-evident
//! [`audit`](crate::audit) log (§IV-D), and — when the call proceeds — post the
//! in-band `m.gauss.agent.tool_call` event to the room timeline (§IV-B). The
//! gateway is the sole channel through which an agent acts: every call is
//! scoped, mediated, auditable, and visible in-band.

use std::sync::Arc;

use gaussmatrix_core::{
	Result, err, implement,
	matrix::{Event, pdu::PduBuilder},
};
use gm_agent::{
	Action, AgentProfile, CAPABILITY_GRANT_TYPE, CapabilityGrant, DEFAULT_AGENT_NAMESPACE, Decision,
	Gateway, Mediation, TOOL_APPROVAL_TYPE, TOOL_CALL_TYPE, TOOL_RESULT_TYPE, ToolApproval,
	ToolCall, ToolResult, handle_mcp, is_agent_id, mcp_call_ack, tool_call_from_mcp,
};
use gm_store::Domain;
use ruma::{OwnedEventId, RoomId, UserId, events::StateEventType};
use serde_json::{Value as JsonValue, value::to_raw_value};

/// Approval state for a call awaiting a human decision (§IV-C).
const APPROVAL_PENDING: &[u8] = b"P";
/// Approval state for a call a human has approved.
const APPROVAL_APPROVED: &[u8] = b"A";
/// Approval state for a call a human has rejected.
const APPROVAL_REJECTED: &[u8] = b"R";

pub struct Service {
	services: Arc<crate::services::OnceServices>,
}

/// The `AgentApprovals` store key for a call: `room_id \0 call_id`.
fn approval_key(room_id: &RoomId, call_id: &str) -> Vec<u8> {
	let room = room_id.as_str().as_bytes();
	let mut key = Vec::with_capacity(room.len().saturating_add(1).saturating_add(call_id.len()));
	key.extend_from_slice(room);
	key.push(0);
	key.extend_from_slice(call_id.as_bytes());
	key
}

impl crate::Service for Service {
	fn build(args: &crate::Args<'_>) -> Result<Arc<Self>> {
		Ok(Arc::new(Self { services: args.services.clone() }))
	}

	fn name(&self) -> &str { crate::service::make_name(std::module_path!()) }
}

/// Read the agent capability grant from a room's `m.gauss.agent.capability`
/// state. A room with no grant yields the default (deny-all) grant.
#[implement(Service)]
pub async fn grant_for(&self, room_id: &RoomId) -> CapabilityGrant {
	let event_type: StateEventType = CAPABILITY_GRANT_TYPE.into();
	match self.services.state_accessor.room_state_get(room_id, &event_type, "").await {
		| Ok(pdu) => pdu
			.get_content::<JsonValue>()
			.map(|content| CapabilityGrant::from_content(&content))
			.unwrap_or_default(),
		| Err(_) => CapabilityGrant::default(),
	}
}

/// Write a room's `m.gauss.agent.capability` grant as a state event (sent by the
/// server user) and append a grant-change record to the audit log — the
/// versioned, auditable edit of the capability lifecycle (§IV-C).
#[implement(Service)]
pub async fn set_grant(&self, room_id: &RoomId, grant: &CapabilityGrant) -> Result<OwnedEventId> {
	let server_user = self.services.globals.server_user.as_ref();
	let builder = PduBuilder {
		event_type: CAPABILITY_GRANT_TYPE.into(),
		content: to_raw_value(&grant.to_content())
			.expect("capability grant content is valid JSON"),
		state_key: Some(String::new().into()),
		..PduBuilder::default()
	};

	let state_lock = self.services.state.mutex.lock(room_id).await;
	let event_id = self
		.services
		.timeline
		.build_and_append_pdu(builder, server_user, room_id, &state_lock)
		.await?;
	drop(state_lock);

	let record = serde_json::to_vec(&serde_json::json!({
		"editor": server_user.as_str(),
		"room": room_id.as_str(),
		"version": grant.version(),
		"action": "grant_set",
	}))
	.unwrap_or_default();
	self.services.audit.append(&record)?;

	Ok(event_id)
}

/// Build a capability grant from operator-supplied specs and [`set_grant`] it.
///
/// `tools` are `name:action` pairs; `default_action` and the explicit `version`
/// are optional. With no version the current grant's version is bumped by one,
/// keeping edits monotonically ordered. Returns the new version.
#[implement(Service)]
pub async fn set_grant_from_spec(
	&self,
	room_id: &RoomId,
	rooms: &[String],
	tools: &[String],
	default_action: Option<&str>,
	version: Option<u64>,
) -> Result<u64> {
	let current = self.grant_for(room_id).await;
	let next = version.unwrap_or_else(|| current.version().saturating_add(1));

	let mut grant = CapabilityGrant::new().with_version(next);
	if let Some(label) = default_action {
		let action = Action::from_label(label).ok_or_else(|| {
			err!(Request(InvalidParam("unknown default action; use auto|review|forbidden")))
		})?;
		grant = grant.with_default_action(action);
	}
	for room in rooms {
		grant = grant.allow_room(room);
	}
	for spec in tools {
		let (name, label) = spec
			.split_once(':')
			.ok_or_else(|| err!(Request(InvalidParam("tool must be given as name:action"))))?;
		let action = Action::from_label(label).ok_or_else(|| {
			err!(Request(InvalidParam("unknown tool action; use auto|review|forbidden")))
		})?;
		grant = grant.allow_tool(name, action);
	}

	self.set_grant(room_id, &grant).await?;
	Ok(next)
}

/// Mediate a tool `call` in `room` against an explicit `grant`, recording the
/// decision in the tamper-evident audit log and returning the mediation outcome.
#[implement(Service)]
pub fn mediate_tool_call(
	&self,
	agent: &str,
	grant: &CapabilityGrant,
	room: &str,
	call: &ToolCall,
) -> Result<Mediation> {
	let mediation = Gateway::new(agent, grant.clone()).process(call, room);
	self.services.audit.append(&mediation.audit_record)?;

	Ok(mediation)
}

/// The full live loop for `agent`'s tool `call` in `room_id`: read the room's
/// grant, mediate, audit, and — when the call proceeds — post the in-band
/// `m.gauss.agent.tool_call` event.
#[implement(Service)]
pub async fn handle_tool_call(
	&self,
	agent: &UserId,
	room_id: &RoomId,
	call: &ToolCall,
) -> Result<Mediation> {
	let grant = self.grant_for(room_id).await;
	let mediation = self.mediate_tool_call(agent.as_str(), &grant, room_id.as_str(), call)?;

	if let Some(event) = &mediation.event {
		self.post_agent_event(agent, room_id, TOOL_CALL_TYPE, event).await?;
	}

	Ok(mediation)
}

/// Run an inbound MCP JSON-RPC `request` for `agent`, scoped to `room_id`'s
/// capability grant — the live MCP gateway (§IV-B).
///
/// A `tools/call` flows through the full mediated loop (decide → audit →
/// in-band `m.gauss.agent.tool_call` when it proceeds) and returns the
/// synchronous MCP acknowledgement; read-only methods (`tools/list`,
/// `resources/list`) return grant-scoped listings. Returns `None` for
/// unrecognised methods.
#[implement(Service)]
pub async fn handle_mcp_request(
	&self,
	agent: &UserId,
	room_id: &RoomId,
	request: &JsonValue,
) -> Result<Option<JsonValue>> {
	let grant = self.grant_for(room_id).await;

	if let Some(call) = tool_call_from_mcp(request) {
		let mediation = self.mediate_tool_call(agent.as_str(), &grant, room_id.as_str(), &call)?;
		if let Some(event) = &mediation.event {
			self.post_agent_event(agent, room_id, TOOL_CALL_TYPE, event).await?;
		}
		// A call needing approval is gated: its result is refused until a human
		// decides (§IV-C). Auto-executed calls leave no gate.
		if mediation.decision == Decision::RequiresApproval {
			self.set_approval_status(room_id, &call.call_id, APPROVAL_PENDING)?;
		}
		return Ok(Some(mcp_call_ack(&call, mediation.decision)));
	}

	Ok(handle_mcp(&grant, request))
}

/// Post a completed tool result in-band as an `m.gauss.agent.tool_result` event.
#[implement(Service)]
pub async fn record_tool_result(
	&self,
	agent: &UserId,
	room_id: &RoomId,
	result: &ToolResult,
) -> Result<OwnedEventId> {
	self.post_agent_event(agent, room_id, TOOL_RESULT_TYPE, &result.to_content()).await
}

/// Ingest a tool result reported over the wire (correlated to its call by
/// `call_id`) and post it in-band — the result half of the gateway loop (§IV-B).
#[implement(Service)]
pub async fn ingest_tool_result(
	&self,
	agent: &UserId,
	room_id: &RoomId,
	content: &JsonValue,
) -> Result<OwnedEventId> {
	let result = ToolResult::from_content(content)
		.ok_or_else(|| err!(Request(InvalidParam("tool result requires a call_id"))))?;

	// Bind execution to approval: a call still awaiting (or refused by) a human
	// reviewer cannot have its result accepted. Calls with no gate (auto-executed
	// or never mediated here) pass through.
	if let Some(status) = self.approval_status(room_id, &result.call_id)? {
		if status.as_slice() == APPROVAL_PENDING {
			return Err(err!(Request(Forbidden("Tool call is awaiting human approval."))));
		}
		if status.as_slice() == APPROVAL_REJECTED {
			return Err(err!(Request(Forbidden("Tool call was rejected by a reviewer."))));
		}
	}

	self.record_tool_result(agent, room_id, &result).await
}

/// Whether `user_id` is in the agent namespace (§IV-A) — a candidate agent
/// principal, as opposed to a human participant. Namespace membership alone does
/// not imply provisioning; see [`is_provisioned`](Self::is_provisioned).
#[implement(Service)]
#[must_use]
pub fn is_agent(&self, user_id: &UserId) -> bool {
	is_agent_id(user_id.as_str(), DEFAULT_AGENT_NAMESPACE)
}

/// Provision an agent identity (§IV-A): validate the namespace and cross-signing
/// key, persist the [`AgentProfile`] in the registry, and audit the action.
/// `operator` is the appservice that owns the agent's namespace.
#[implement(Service)]
pub fn provision_agent(
	&self,
	agent_id: &UserId,
	operator: &str,
	signing_key: &str,
	display_name: Option<&str>,
) -> Result<AgentProfile> {
	let profile = AgentProfile::provision(
		agent_id.as_str(),
		DEFAULT_AGENT_NAMESPACE,
		operator,
		signing_key,
		display_name,
	)
	.map_err(|e| {
		let reason = e.label();
		err!(Request(InvalidParam("cannot provision agent: {reason}")))
	})?;

	let value = serde_json::to_vec(&profile.to_content())
		.map_err(|e| err!(Database("agent profile serialization failed: {e}")))?;
	self.services
		.store
		.put(Domain::AgentRegistry, agent_id.as_str().as_bytes().to_vec(), value)
		.map_err(|e| err!(Database("agent registry write failed: {e}")))?;

	let record = serde_json::to_vec(&serde_json::json!({
		"operator": operator,
		"agent": agent_id.as_str(),
		"action": "provision",
	}))
	.unwrap_or_default();
	self.services.audit.append(&record)?;

	Ok(profile)
}

/// The provisioning record for `agent_id`, if it has been provisioned.
#[implement(Service)]
pub fn agent_profile(&self, agent_id: &UserId) -> Result<Option<AgentProfile>> {
	let stored = self
		.services
		.store
		.get(Domain::AgentRegistry, agent_id.as_str().as_bytes())
		.map_err(|e| err!(Database("agent registry read failed: {e}")))?;

	let Some(bytes) = stored else {
		return Ok(None);
	};
	let content: JsonValue = serde_json::from_slice(&bytes)
		.map_err(|e| err!(Database("agent profile is corrupt: {e}")))?;

	Ok(AgentProfile::from_content(&content))
}

/// Whether `agent_id` has been provisioned through the registry (§IV-A).
#[implement(Service)]
pub fn is_provisioned(&self, agent_id: &UserId) -> Result<bool> {
	self.services
		.store
		.contains(Domain::AgentRegistry, agent_id.as_str().as_bytes())
		.map_err(|e| err!(Database("agent registry lookup failed: {e}")))
}

/// Remove an agent's provisioning record, auditing the action. Returns whether
/// a record existed.
#[implement(Service)]
pub fn deprovision_agent(&self, agent_id: &UserId) -> Result<bool> {
	if !self.is_provisioned(agent_id)? {
		return Ok(false);
	}

	self.services
		.store
		.delete(Domain::AgentRegistry, agent_id.as_str().as_bytes().to_vec())
		.map_err(|e| err!(Database("agent registry delete failed: {e}")))?;

	let record = serde_json::to_vec(&serde_json::json!({
		"agent": agent_id.as_str(),
		"action": "deprovision",
	}))
	.unwrap_or_default();
	self.services.audit.append(&record)?;

	Ok(true)
}

/// Every provisioned agent profile, for operator inspection.
#[implement(Service)]
pub fn provisioned_agents(&self) -> Result<Vec<AgentProfile>> {
	let scanned = self
		.services
		.store
		.prefix_scan(Domain::AgentRegistry, b"")
		.map_err(|e| err!(Database("agent registry scan failed: {e}")))?;

	let mut out = Vec::with_capacity(scanned.len());
	for (_key, value) in scanned {
		let content: JsonValue = serde_json::from_slice(&value)
			.map_err(|e| err!(Database("agent profile is corrupt: {e}")))?;
		if let Some(profile) = AgentProfile::from_content(&content) {
			out.push(profile);
		}
	}

	Ok(out)
}

/// Record a human-in-the-loop approval decision on a tool call that required
/// approval (§IV-C): append it to the audit log and post it in-band as an
/// `m.gauss.agent.tool_approval`, correlated to the call by `call_id`.
#[implement(Service)]
pub async fn record_approval(
	&self,
	reviewer: &UserId,
	room_id: &RoomId,
	call_id: &str,
	approved: bool,
	reason: Option<&str>,
) -> Result<OwnedEventId> {
	let approval = ToolApproval::new(call_id, approved, reviewer.as_str(), reason);
	self.services.audit.append(&approval.audit_record())?;

	// Record the decision so the matching tool result is admitted or refused.
	let status = if approved { APPROVAL_APPROVED } else { APPROVAL_REJECTED };
	self.set_approval_status(room_id, call_id, status)?;

	self.post_agent_event(reviewer, room_id, TOOL_APPROVAL_TYPE, &approval.to_content())
		.await
}

/// Persist the approval `status` for a call (`AgentApprovals` domain).
#[implement(Service)]
fn set_approval_status(&self, room_id: &RoomId, call_id: &str, status: &[u8]) -> Result<()> {
	self.services
		.store
		.put(Domain::AgentApprovals, approval_key(room_id, call_id), status.to_vec())
		.map_err(|e| err!(Database("agent approval write failed: {e}")))
}

/// Read the persisted approval status for a call, if any.
#[implement(Service)]
fn approval_status(&self, room_id: &RoomId, call_id: &str) -> Result<Option<Vec<u8>>> {
	self.services
		.store
		.get(Domain::AgentApprovals, &approval_key(room_id, call_id))
		.map_err(|e| err!(Database("agent approval read failed: {e}")))
}

/// Build and append a namespaced agent event to the room timeline.
#[implement(Service)]
async fn post_agent_event(
	&self,
	sender: &UserId,
	room_id: &RoomId,
	event_type: &str,
	content: &JsonValue,
) -> Result<OwnedEventId> {
	let builder = PduBuilder {
		event_type: event_type.into(),
		content: to_raw_value(content).expect("agent event content is valid JSON"),
		..PduBuilder::default()
	};

	let state_lock = self.services.state.mutex.lock(room_id).await;
	self.services
		.timeline
		.build_and_append_pdu(builder, sender, room_id, &state_lock)
		.await
}
