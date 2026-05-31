use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn trim_memory(&self) -> Result {
	gaussmatrix_core::alloc::trim(None)?;

	writeln!(self, "done").await
}
