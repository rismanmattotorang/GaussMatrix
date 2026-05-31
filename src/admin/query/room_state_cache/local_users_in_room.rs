use futures::StreamExt;
use ruma::OwnedRoomId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn local_users_in_room(&self, room_id: OwnedRoomId) -> Result {
	self.write_timed_query(
		self.services
			.state_cache
			.local_users_in_room(&room_id)
			.map(ToOwned::to_owned)
			.collect::<Vec<_>>(),
	)
	.await
}
