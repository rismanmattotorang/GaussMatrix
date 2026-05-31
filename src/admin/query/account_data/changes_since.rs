use futures::StreamExt;
use ruma::{OwnedRoomId, OwnedUserId};
use tokio::time::Instant;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn changes_since(
	&self,
	user_id: OwnedUserId,
	since: u64,
	room_id: Option<OwnedRoomId>,
) -> Result {
	let timer = Instant::now();
	let results: Vec<_> = self
		.services
		.account_data
		.changes_since(room_id.as_deref(), &user_id, since, None)
		.collect()
		.await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{results:?}\n```").await
}
