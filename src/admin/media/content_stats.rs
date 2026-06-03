use gaussmatrix_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn content_stats(&self) -> Result {
	let blobs = self.services.cas.blob_count()?;

	write!(self, "Content-addressed media store: {blobs} distinct blob(s).").await
}
