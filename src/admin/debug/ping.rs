use ruma::{OwnedServerName, api::federation::discovery::get_server_version};
use tokio::time::Instant;
use tuwunel_core::{Result, err};

use crate::admin_command;

#[admin_command]
pub(super) async fn ping(&self, server: OwnedServerName) -> Result {
	let timer = Instant::now();

	let response = self
		.services
		.federation
		.execute(&server, get_server_version::v1::Request {})
		.await
		.map_err(|e| err!("Failed sending federation request to specified server:\n\n{e}"))?;

	let ping_time = timer.elapsed();

	let out = if let Ok(json) = serde_json::to_string_pretty(&response.server) {
		format!("Got response which took {ping_time:?} time:\n```json\n{json}\n```")
	} else {
		format!("Got non-JSON response which took {ping_time:?} time:\n{response:?}")
	};

	self.write_str(&out).await
}
