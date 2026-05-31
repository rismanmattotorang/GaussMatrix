#![cfg(test)]

use insta::{assert_debug_snapshot, with_settings};
use tracing::Level;
use gaussmatrix::{Args, Runtime, Server};
use gaussmatrix_core::{Result, utils::result::ErrLog};

#[test]
fn smoke_shutdown() -> Result {
	with_settings!({
		description => "Smoke Shutdown",
		snapshot_suffix => "smoke_shutdown",
	}, {
		let args = Args::default_test(&["fresh", "cleanup"]);
		let runtime = Runtime::new(Some(&args))?;
		let server = Server::new(Some(&args), Some(&runtime))?;
		let result = runtime.block_on(async {
			gaussmatrix::async_start(&server).await?;
			let run = gaussmatrix::async_run(&server);
			server.server.shutdown().log_err(Level::WARN).ok();
			run.await?;
			gaussmatrix::async_stop(&server).await
		});

		drop(runtime);
		assert_debug_snapshot!(result);
		result
	})
}
