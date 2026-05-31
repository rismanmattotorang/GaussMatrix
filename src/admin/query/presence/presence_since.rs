use futures::StreamExt;
use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn presence_presence_since(&self, since: u64, to: Option<u64>) -> Result {
	let timer = Instant::now();
	let results: Vec<(_, _, _)> = self
		.services
		.presence
		.presence_since(since, to)
		.map(|(user_id, count, bytes)| (user_id.to_owned(), count, bytes.to_vec()))
		.collect()
		.await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{results:#?}\n```").await
}
