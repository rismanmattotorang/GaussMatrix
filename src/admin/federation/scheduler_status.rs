use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn scheduler_status(&self) -> Result {
	let depths = self.services.fed.queue_depths();
	let failing = self.services.fed.failing().await;

	write!(self, "Outbound scheduler (gm-fed):\n").await?;

	write!(self, "Queued destinations:\n").await?;
	if depths.is_empty() {
		write!(self, "- (none)\n").await?;
	} else {
		for (destination, depth) in depths {
			write!(self, "- {destination}: {depth}\n").await?;
		}
	}

	write!(self, "Destinations in backoff (consecutive failures):\n").await?;
	if failing.is_empty() {
		write!(self, "- (none)\n").await?;
	} else {
		for (destination, attempts, _available_at) in failing {
			write!(self, "- {destination}: {attempts} failure(s)\n").await?;
		}
	}

	Ok(())
}
