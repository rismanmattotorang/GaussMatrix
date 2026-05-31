use std::sync::Arc;

use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn backup_database(&self) -> Result {
	let db = Arc::clone(&self.services.db);
	let result = self
		.services
		.server
		.runtime()
		.spawn_blocking(move || match db.engine.backup() {
			| Ok(()) => "Done".to_owned(),
			| Err(e) => format!("Failed: {e}"),
		})
		.await?;

	let count = self.services.db.engine.backup_count()?;
	write!(self, "{result}. Currently have {count} backups.").await
}
