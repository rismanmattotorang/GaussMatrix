use gaussmatrix_core::Result;
use ruma::{OwnedRoomId, OwnedUserId};

use crate::admin_command;

#[admin_command]
pub(super) async fn agent_quota(&self, user_id: OwnedUserId, room_id: OwnedRoomId) -> Result {
	let quotas = self.services.agent.quota(&user_id, &room_id).await?;

	if quotas.is_empty() {
		return write!(self, "No rate-limited tools for {user_id} in {room_id}.").await;
	}

	write!(self, "Rate-limit quota for {user_id} in {room_id}:\n").await?;
	for q in quotas {
		write!(
			self,
			"- {}: {}/{} used, {} remaining (window {}s, resets in {}s)\n",
			q.tool, q.used, q.max, q.remaining, q.window_secs, q.resets_in_secs,
		)
		.await?;
	}

	Ok(())
}
