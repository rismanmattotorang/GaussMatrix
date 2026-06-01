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
	Result, implement,
	matrix::{Event, pdu::PduBuilder},
};
use gm_agent::{
	CAPABILITY_GRANT_TYPE, CapabilityGrant, Gateway, Mediation, TOOL_CALL_TYPE, TOOL_RESULT_TYPE,
	ToolCall, ToolResult,
};
use ruma::{OwnedEventId, RoomId, UserId, events::StateEventType};
use serde_json::{Value as JsonValue, value::to_raw_value};

pub struct Service {
	services: Arc<crate::services::OnceServices>,
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
