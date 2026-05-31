use futures::StreamExt;
use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn all_local_aliases(&self) -> Result {
	let timer = Instant::now();
	let aliases = self
		.services
		.alias
		.all_local_aliases()
		.map(|(room_id, alias)| (room_id.to_owned(), alias.to_owned()))
		.collect::<Vec<_>>()
		.await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{aliases:#?}\n```").await
}
