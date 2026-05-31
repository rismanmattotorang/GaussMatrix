use futures::StreamExt;
use ruma::OwnedUserId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn rooms_left(&self, user_id: OwnedUserId) -> Result {
	self.write_timed_query(
		self.services
			.state_cache
			.rooms_left_state(&user_id)
			.collect::<Vec<_>>(),
	)
	.await
}
