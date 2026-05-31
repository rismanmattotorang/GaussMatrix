use futures::StreamExt;
use tokio::time::Instant;
use tuwunel_core::{
	Result,
	utils::{
		stream::{IterStream, ReadyExt},
		string::EMPTY,
	},
};

use super::with_map_or;
use crate::admin_command;

#[admin_command]
pub(super) async fn raw_count(&self, map: Option<String>, prefix: Option<String>) -> Result {
	let prefix = prefix.as_deref().unwrap_or(EMPTY);

	let timer = Instant::now();
	let count = with_map_or(map.as_deref(), self.services)?
		.iter()
		.stream()
		.then(|map| map.raw_count_prefix(&prefix))
		.ready_fold(0_usize, usize::saturating_add)
		.await;

	let query_time = timer.elapsed();
	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{count:#?}\n```").await
}
