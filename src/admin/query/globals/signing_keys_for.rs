use ruma::OwnedServerName;
use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn globals_signing_keys_for(&self, origin: OwnedServerName) -> Result {
	let timer = Instant::now();
	let results = self
		.services
		.server_keys
		.verify_keys_for(&origin)
		.await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{results:#?}\n```").await
}
