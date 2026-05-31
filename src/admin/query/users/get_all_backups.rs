use ruma::OwnedUserId;
use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_all_backups(&self, user_id: OwnedUserId, version: String) -> Result {
	let timer = Instant::now();
	let result = self
		.services
		.key_backups
		.get_all(&user_id, &version)
		.await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{result:#?}\n```").await
}
