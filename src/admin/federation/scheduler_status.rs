use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn scheduler_status(&self) -> Result {
	let depths = self.services.fed.queue_depths().await;

	if depths.is_empty() {
		return write!(self, "Outbound scheduler (gm-fed): no queued destinations.").await;
	}

	write!(self, "Outbound scheduler (gm-fed) queue depths:\n").await?;
	for (destination, depth) in depths {
		write!(self, "- {destination}: {depth}\n").await?;
	}

	Ok(())
}
