use ruma::{OwnedRoomId, OwnedUserId};
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_invite_count(
	&self,
	room_id: OwnedRoomId,
	user_id: OwnedUserId,
) -> Result {
	self.write_timed_query(
		self.services
			.state_cache
			.get_invite_count(&room_id, &user_id),
	)
	.await
}
