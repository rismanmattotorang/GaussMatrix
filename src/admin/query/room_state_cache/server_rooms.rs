use futures::StreamExt;
use ruma::OwnedServerName;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn server_rooms(&self, server: OwnedServerName) -> Result {
	self.write_timed_query(
		self.services
			.state_cache
			.server_rooms(&server)
			.map(ToOwned::to_owned)
			.collect::<Vec<_>>(),
	)
	.await
}
