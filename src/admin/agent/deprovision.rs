use gaussmatrix_core::Result;
use ruma::OwnedUserId;

use crate::admin_command;

#[admin_command]
pub(super) async fn agent_deprovision(&self, user_id: OwnedUserId) -> Result {
	if self.services.agent.deprovision_agent(&user_id)? {
		write!(self, "Deprovisioned agent `{user_id}`.").await
	} else {
		write!(self, "No provisioned agent `{user_id}`.").await
	}
}
