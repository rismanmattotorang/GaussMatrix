use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn agent_audit_sign(&self) -> Result {
	let manifest = self.services.agent.sign_audit_export()?;

	write!(self, "Server-signed audit manifest:\n```json\n{manifest}\n```").await
}
