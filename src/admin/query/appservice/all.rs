use futures::TryStreamExt;
use tokio::time::Instant;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn appservice_all(&self) -> Result {
	let timer = Instant::now();
	let results: Vec<_> = self
		.services
		.appservice
		.iter_db_ids()
		.try_collect()
		.await?;

	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{results:#?}\n```").await
}
