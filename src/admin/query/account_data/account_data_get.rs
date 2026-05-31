use ruma::{OwnedRoomId, OwnedUserId};
use tokio::time::Instant;
use gaussmatrix_core::Result;
use gaussmatrix_database::Deserialized;

use crate::admin_command;

#[admin_command]
pub(super) async fn account_data_get(
	&self,
	user_id: OwnedUserId,
	kind: String,
	room_id: Option<OwnedRoomId>,
) -> Result {
	let timer = Instant::now();
	let results: serde_json::Value = self
		.services
		.account_data
		.get_raw(room_id.as_deref(), &user_id, &kind)
		.await
		.deserialized()?;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{results:#?}\n```").await
}
