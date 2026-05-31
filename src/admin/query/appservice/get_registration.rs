use tokio::time::Instant;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn appservice_get_registration(&self, appservice_id: String) -> Result {
	let timer = Instant::now();
	let results = self
		.services
		.appservice
		.get_registration(&appservice_id)
		.await;

	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{results:#?}\n```").await
}
