use ruma::{OwnedRoomId, OwnedUserId};
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn invite_state(&self, user_id: OwnedUserId, room_id: OwnedRoomId) -> Result {
	self.write_timed_query(
		self.services
			.state_cache
			.invite_state(&user_id, &room_id),
	)
	.await
}
