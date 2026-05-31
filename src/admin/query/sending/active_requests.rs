use futures::StreamExt;
use tokio::time::Instant;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn sending_active_requests(&self) -> Result {
	let timer = Instant::now();
	let results = self.services.sending.db.active_requests();
	let active_requests = results.collect::<Vec<_>>().await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{active_requests:#?}\n```").await
}
