use futures::TryStreamExt;
use gaussmatrix_core::{Result, utils::stream::IterStream};

use crate::admin_command;

#[admin_command]
pub(super) async fn list_backups(&self) -> Result {
	self.services
		.db
		.engine
		.backup_list()?
		.try_stream()
		.try_for_each(|result| write!(self, "{result}"))
		.await
}
