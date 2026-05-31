use futures::StreamExt;
use ruma::OwnedUserId;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn rooms_invited(&self, user_id: OwnedUserId) -> Result {
	self.write_timed_query(
		self.services
			.state_cache
			.rooms_invited_state(&user_id)
			.collect::<Vec<_>>(),
	)
	.await
}
