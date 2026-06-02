use gaussmatrix_core::Result;
use ruma::OwnedUserId;

use crate::admin_command;

#[admin_command]
pub(super) async fn agent_profile(&self, user_id: OwnedUserId) -> Result {
	match self.services.agent.agent_profile(&user_id)? {
		| Some(profile) =>
			write!(
				self,
				"Agent `{}`\n- operator: {}\n- signing key: {}\n- display name: {}",
				profile.agent_id,
				profile.operator,
				profile.signing_key,
				profile.display_name.as_deref().unwrap_or("(none)"),
			)
			.await,
		| None => write!(self, "No provisioned agent `{user_id}`.").await,
	}
}
