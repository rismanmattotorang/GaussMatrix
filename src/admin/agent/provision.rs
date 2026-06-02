use gaussmatrix_core::Result;
use ruma::OwnedUserId;

use crate::admin_command;

#[admin_command]
pub(super) async fn agent_provision(
	&self,
	user_id: OwnedUserId,
	signing_key: String,
	display_name: Option<String>,
) -> Result {
	let profile = self.services.agent.provision_agent(
		&user_id,
		"admin-console",
		&signing_key,
		display_name.as_deref(),
	)?;

	write!(
		self,
		"Provisioned agent `{}` (operator: {}, display name: {}).",
		profile.agent_id,
		profile.operator,
		profile.display_name.as_deref().unwrap_or("(none)"),
	)
	.await
}
