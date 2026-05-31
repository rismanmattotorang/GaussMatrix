use futures::stream::StreamExt;
use ruma::OwnedUserId;
use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn iter_users(&self) -> Result {
	let timer = Instant::now();
	let result: Vec<OwnedUserId> = self
		.services
		.users
		.stream()
		.map(Into::into)
		.collect()
		.await;

	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{result:#?}\n```").await
}
