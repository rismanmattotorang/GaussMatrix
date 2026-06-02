use gaussmatrix_core::Result;
use ruma::OwnedRoomId;

use crate::admin_command;

#[admin_command]
pub(super) async fn agent_grant_set(
	&self,
	room_id: OwnedRoomId,
	tools: Vec<String>,
	rooms: Vec<String>,
	rates: Vec<String>,
	default_action: Option<String>,
	version: Option<u64>,
) -> Result {
	let new_version = self
		.services
		.agent
		.set_grant_from_spec(&room_id, &rooms, &tools, &rates, default_action.as_deref(), version)
		.await?;

	write!(self, "Capability grant for {room_id} updated to version {new_version}.").await
}
