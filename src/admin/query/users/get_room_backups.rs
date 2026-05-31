use ruma::{OwnedRoomId, OwnedUserId};
use tokio::time::Instant;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_room_backups(
	&self,
	user_id: OwnedUserId,
	version: String,
	room_id: OwnedRoomId,
) -> Result {
	let timer = Instant::now();
	let result = self
		.services
		.key_backups
		.get_room(&user_id, &version, &room_id)
		.await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{result:#?}\n```").await
}
