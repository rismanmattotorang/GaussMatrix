use futures::StreamExt;
use ruma::OwnedUserId;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn rooms_joined(&self, user_id: OwnedUserId) -> Result {
	self.write_timed_query(
		self.services
			.state_cache
			.rooms_joined(&user_id)
			.map(ToOwned::to_owned)
			.collect::<Vec<_>>(),
	)
	.await
}
