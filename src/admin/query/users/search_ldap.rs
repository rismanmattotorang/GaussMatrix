use ruma::OwnedUserId;
use tokio::time::Instant;
use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn search_ldap(&self, user_id: OwnedUserId) -> Result {
	let timer = Instant::now();
	let result = self.services.users.search_ldap(&user_id).await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{result:#?}\n```").await
}
