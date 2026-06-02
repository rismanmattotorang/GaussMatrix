use gaussmatrix_core::Result;
use ruma::OwnedRoomId;

use crate::admin_command;

#[admin_command]
pub(super) async fn agent_grant_show(&self, room_id: OwnedRoomId) -> Result {
	let grant = self.services.agent.grant_for(&room_id).await;

	let rooms: Vec<&str> = grant.rooms().collect();
	let tools: Vec<String> =
		grant.tools().map(|(name, action)| format!("{name}:{}", action.label())).collect();
	let rates: Vec<String> = grant
		.rate_limits()
		.map(|(tool, limit)| format!("{tool}:{}/{}s", limit.max, limit.window_secs))
		.collect();

	write!(
		self,
		"Capability grant for {room_id} (version {}):\n- rooms: {}\n- tools: {}\n- rate limits: {}",
		grant.version(),
		if rooms.is_empty() { "(none)".to_owned() } else { rooms.join(", ") },
		if tools.is_empty() { "(none)".to_owned() } else { tools.join(", ") },
		if rates.is_empty() { "(none)".to_owned() } else { rates.join(", ") },
	)
	.await
}
