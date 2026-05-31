use futures::stream::StreamExt;
use ruma::OwnedUserId;
use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_shared_rooms(&self, user_a: OwnedUserId, user_b: OwnedUserId) -> Result {
	let timer = Instant::now();
	let result: Vec<_> = self
		.services
		.state_cache
		.get_shared_rooms(&user_a, &user_b)
		.map(ToOwned::to_owned)
		.collect()
		.await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{result:#?}\n```").await
}
