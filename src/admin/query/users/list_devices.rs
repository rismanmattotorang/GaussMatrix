use futures::stream::StreamExt;
use ruma::OwnedUserId;
use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn list_devices(&self, user_id: OwnedUserId) -> Result {
	let timer = Instant::now();
	let devices = self
		.services
		.users
		.all_device_ids(&user_id)
		.map(ToOwned::to_owned)
		.collect::<Vec<_>>()
		.await;

	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{devices:#?}\n```").await
}
