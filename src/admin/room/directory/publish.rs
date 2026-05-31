use ruma::OwnedRoomId;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn directory_publish(&self, room_id: OwnedRoomId) -> Result {
	self.services.directory.set_public(&room_id);
	self.write_str("Room published").await
}
