use ruma::OwnedServerName;
use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn sending_get_latest_edu_count(&self, server_name: OwnedServerName) -> Result {
	let timer = Instant::now();
	let results = self
		.services
		.sending
		.db
		.get_latest_educount(&server_name)
		.await;

	let query_time = timer.elapsed();
	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{results:#?}\n```").await
}
