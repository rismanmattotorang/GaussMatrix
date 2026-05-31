use tokio::time::Instant;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn raw_del(&self, map: String, key: String) -> Result {
	let map = self.services.db.get(&map)?;
	let timer = Instant::now();
	map.remove(&key);

	let query_time = timer.elapsed();
	write!(self, "Operation completed in {query_time:?}").await
}
