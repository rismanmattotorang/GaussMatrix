use ruma::OwnedUserId;
use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_latest_backup(&self, user_id: OwnedUserId) -> Result {
	let timer = Instant::now();
	let result = self
		.services
		.key_backups
		.get_latest_backup(&user_id)
		.await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{result:#?}\n```").await
}
