use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn agent_audit_verify(&self) -> Result {
	match self.services.audit.verify() {
		| Ok(()) =>
			write!(self, "Audit log integrity verified: the hash chain is intact.").await,
		| Err(e) => write!(self, "Audit log verification FAILED: {e}").await,
	}
}
