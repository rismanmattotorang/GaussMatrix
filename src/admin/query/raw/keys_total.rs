use futures::StreamExt;
use tokio::time::Instant;
use gaussmatrix_core::{
	Result,
	utils::{
		stream::{IterStream, ReadyExt, TryIgnore},
		string::EMPTY,
	},
};

use super::with_map_or;
use crate::admin_command;

#[admin_command]
pub(super) async fn raw_keys_total(&self, map: Option<String>, prefix: Option<String>) -> Result {
	let prefix = prefix.as_deref().unwrap_or(EMPTY);

	let timer = Instant::now();
	let result = with_map_or(map.as_deref(), self.services)?
		.iter()
		.stream()
		.map(|map| map.raw_keys_prefix(&prefix))
		.flatten()
		.ignore_err()
		.map(<[u8]>::len)
		.ready_fold_default(|acc: usize, len| acc.saturating_add(len))
		.await;

	let query_time = timer.elapsed();
	write!(self, "```\n{result:#?}\n\n```\n\nQuery completed in {query_time:?}").await
}
