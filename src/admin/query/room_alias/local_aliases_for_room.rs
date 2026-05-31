use futures::StreamExt;
use ruma::OwnedRoomId;
use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn local_aliases_for_room(&self, room_id: OwnedRoomId) -> Result {
	let timer = Instant::now();
	let aliases: Vec<_> = self
		.services
		.alias
		.local_aliases_for_room(&room_id)
		.map(ToOwned::to_owned)
		.collect()
		.await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{aliases:#?}\n```").await
}
