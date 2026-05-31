use ruma::OwnedUserId;
use tokio::time::Instant;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_devices_version(&self, user_id: OwnedUserId) -> Result {
	let timer = Instant::now();
	let device = self
		.services
		.users
		.get_devicelist_version(&user_id)
		.await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{device:#?}\n```").await
}
