use gaussmatrix_core::Result;
use ruma::OwnedRoomId;

use crate::admin_command;

#[admin_command]
pub(super) async fn agent_grant_show(&self, room_id: OwnedRoomId) -> Result {
	let grant = self.services.agent.grant_for(&room_id).await;

	let rooms: Vec<&str> = grant.rooms().collect();
	let tools: Vec<String> =
		grant.tools().map(|(name, action)| format!("{name}:{}", action.label())).collect();

	write!(
		self,
		"Capability grant for {room_id}:\n- rooms: {}\n- tools: {}",
		if rooms.is_empty() { "(none)".to_owned() } else { rooms.join(", ") },
		if tools.is_empty() { "(none)".to_owned() } else { tools.join(", ") },
	)
	.await
}
