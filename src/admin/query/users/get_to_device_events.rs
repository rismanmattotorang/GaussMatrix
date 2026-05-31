use futures::stream::StreamExt;
use ruma::{OwnedDeviceId, OwnedUserId};
use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_to_device_events(
	&self,
	user_id: OwnedUserId,
	device_id: OwnedDeviceId,
) -> Result {
	let timer = Instant::now();
	let result = self
		.services
		.users
		.get_to_device_events(&user_id, &device_id, None, None)
		.collect::<Vec<_>>()
		.await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{result:#?}\n```").await
}
