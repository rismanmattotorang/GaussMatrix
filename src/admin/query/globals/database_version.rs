use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn globals_database_version(&self) -> Result {
	let timer = Instant::now();
	let results = self.services.globals.db.database_version().await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{results:#?}\n```").await
}
