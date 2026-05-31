use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn trim_memory(&self) -> Result {
	tuwunel_core::alloc::trim(None)?;

	writeln!(self, "done").await
}
