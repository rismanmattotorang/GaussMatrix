use ruma::OwnedRoomAliasId;
use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn resolve_alias(&self, alias: OwnedRoomAliasId) -> Result {
	let timer = Instant::now();
	let results = self.services.alias.resolve_alias(&alias).await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{results:#?}\n```").await
}
