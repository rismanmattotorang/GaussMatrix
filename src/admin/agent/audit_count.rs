use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn agent_audit_count(&self) -> Result {
	let count = self.services.audit.count()?;

	write!(self, "Audit log entries: {count}").await
}
