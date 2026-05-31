use tuwunel_core::Result;

use crate::admin_command;

#[cfg(tokio_unstable)]
#[admin_command]
pub(super) async fn runtime_interval(&self) -> Result {
	let out = self
		.services
		.server
		.metrics
		.runtime_interval()
		.map_or_else(
			|| "Runtime metrics are not available.".to_owned(),
			|metrics| format!("```rs\n{metrics:#?}\n```"),
		);

	self.write_str(&out).await
}

#[cfg(not(tokio_unstable))]
#[admin_command]
pub(super) async fn runtime_interval(&self) -> Result {
	self.write_str("Runtime metrics require building with `tokio_unstable`.")
		.await
}
