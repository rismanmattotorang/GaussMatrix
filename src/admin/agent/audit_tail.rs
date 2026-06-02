use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn agent_audit_tail(&self, n: Option<usize>) -> Result {
	let entries = self.services.audit.tail(n.unwrap_or(20))?;

	write!(self, "Last {} audit entries:\n", entries.len()).await?;
	for (seq, payload) in entries {
		let text = String::from_utf8_lossy(&payload);
		write!(self, "- #{seq}: {text}\n").await?;
	}

	Ok(())
}
