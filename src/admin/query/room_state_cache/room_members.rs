use futures::StreamExt;
use ruma::OwnedRoomId;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn room_members(&self, room_id: OwnedRoomId) -> Result {
	self.write_timed_query(
		self.services
			.state_cache
			.room_members(&room_id)
			.map(ToOwned::to_owned)
			.collect::<Vec<_>>(),
	)
	.await
}
