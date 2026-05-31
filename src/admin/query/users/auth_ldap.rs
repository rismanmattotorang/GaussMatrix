use tokio::time::Instant;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn auth_ldap(&self, user_dn: String, password: String) -> Result {
	let timer = Instant::now();
	let result = self
		.services
		.users
		.auth_ldap(&user_dn, &password)
		.await;
	let query_time = timer.elapsed();

	write!(self, "Query completed in {query_time:?}:\n\n```rs\n{result:#?}\n```").await
}
