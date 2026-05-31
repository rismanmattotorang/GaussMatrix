use ruma::{OwnedDeviceId, OwnedUserId};
use tokio::time::Instant;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_device_keys(
	&self,
	user_id: OwnedUserId,
	device_id: OwnedDeviceId,
) -> Result {
	let timer = Instant::now();
	let result = self
		.services
		.users
		.get_device_keys(&user_id, &device_id)
		.await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{result:#?}\n```").await
}
