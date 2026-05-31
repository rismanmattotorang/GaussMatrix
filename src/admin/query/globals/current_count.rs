use tokio::time::Instant;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn globals_current_count(&self) -> Result {
	let timer = Instant::now();
	let results = self.services.globals.current_count();
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{results:#?}\n```").await
}
