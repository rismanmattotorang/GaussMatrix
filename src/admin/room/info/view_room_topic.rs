use ruma::OwnedRoomId;
use gaussmatrix_core::{Err, Result};

use crate::admin_command;

#[admin_command]
pub(super) async fn view_room_topic(&self, room_id: OwnedRoomId) -> Result {
	let Ok(room_topic) = self
		.services
		.state_accessor
		.get_room_topic(&room_id)
		.await
	else {
		return Err!("Room does not have a room topic set.");
	};

	write!(self, "Room topic:\n```\n{room_topic}\n```").await
}
