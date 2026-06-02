use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn agent_audit_export(&self) -> Result {
	let export = self.services.audit.export_jsonl()?;

	write!(self, "```jsonl\n{export}```").await
}
