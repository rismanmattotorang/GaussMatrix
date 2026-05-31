use futures::StreamExt;
use ruma::{OwnedServerName, OwnedUserId};
use tokio::time::Instant;
use tuwunel_core::Result;

use super::parse_destination;
use crate::admin_command;

#[admin_command]
pub(super) async fn sending_active_requests_for(
	&self,
	appservice_id: Option<String>,
	server_name: Option<OwnedServerName>,
	user_id: Option<OwnedUserId>,
	push_key: Option<String>,
) -> Result {
	let destination = parse_destination(appservice_id, server_name, user_id, push_key)?;

	let timer = Instant::now();
	let results = self
		.services
		.sending
		.db
		.active_requests_for(&destination);

	let active_requests = results.collect::<Vec<_>>().await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{active_requests:#?}\n```").await
}
