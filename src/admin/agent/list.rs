use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn agent_list(&self) -> Result {
	let agents = self.services.agent.provisioned_agents()?;

	write!(self, "Provisioned agents: {}\n", agents.len()).await?;
	for agent in agents {
		write!(self, "- `{}` (operator: {})\n", agent.agent_id, agent.operator).await?;
	}

	Ok(())
}
