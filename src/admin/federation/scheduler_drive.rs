use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn scheduler_drive(&self) -> Result {
	let driven = self.services.fed.drive_once().await?;

	write!(self, "gm-fed drive cycle: flushed {driven} ready destination(s).").await
}
