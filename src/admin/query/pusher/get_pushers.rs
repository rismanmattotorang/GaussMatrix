use ruma::OwnedUserId;
use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_pushers(&self, user_id: OwnedUserId) -> Result {
	let timer = Instant::now();
	let results = self.services.pusher.get_pushers(&user_id).await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{results:#?}```").await
}
