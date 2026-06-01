//! The agentic gateway service — the live mediation loop (SPECS §IV).
//!
//! This wires the storage- and transport-pure `gm-agent` policy core into the
//! running server: an agent's tool call is mediated against its
//! [`CapabilityGrant`], the decision is appended to the live tamper-evident
//! [`audit`](crate::audit) log (§IV-D), and the [`Mediation`] (decision plus the
//! in-band `m.gauss.agent.tool_call` event to post) is returned to the caller.
//!
//! The gateway is the sole channel through which an agent acts: every call is
//! scoped, mediated, and auditable. Reading the grant from room state and
//! posting the in-band events to the timeline are the next integration steps;
//! this lands the gateway → audit-log loop.

use std::sync::Arc;

use gaussmatrix_core::{Result, implement};
use gm_agent::{CapabilityGrant, Gateway, Mediation, ToolCall};

pub struct Service {
	services: Arc<crate::services::OnceServices>,
}

impl crate::Service for Service {
	fn build(args: &crate::Args<'_>) -> Result<Arc<Self>> {
		Ok(Arc::new(Self { services: args.services.clone() }))
	}

	fn name(&self) -> &str { crate::service::make_name(std::module_path!()) }
}

/// Mediate `agent`'s tool `call` in `room` against `grant`, recording the
/// decision in the tamper-evident audit log and returning the mediation outcome
/// (decision plus the in-band event to post when the call proceeds).
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
