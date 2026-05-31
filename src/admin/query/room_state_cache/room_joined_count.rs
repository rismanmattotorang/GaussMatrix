use ruma::OwnedRoomId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn room_joined_count(&self, room_id: OwnedRoomId) -> Result {
	self.write_timed_query(
		self.services
			.state_cache
			.room_joined_count(&room_id),
	)
	.await
}
