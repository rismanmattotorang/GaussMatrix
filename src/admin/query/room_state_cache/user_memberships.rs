use futures::StreamExt;
use ruma::OwnedUserId;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn user_memberships(&self, user_id: OwnedUserId) -> Result {
	self.write_timed_query(
		self.services
			.state_cache
			.all_user_memberships(&user_id)
			.map(|(membership, room_id)| (membership, room_id.to_owned()))
			.collect::<Vec<_>>(),
	)
	.await
}
