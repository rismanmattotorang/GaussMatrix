#![cfg(test)]

use insta::{assert_debug_snapshot, with_settings};
use gaussmatrix::{Args, Runtime, Server};
use gaussmatrix_core::Result;

#[test]
fn smoke_async() -> Result {
	with_settings!({
		description => "Smoke Async",
		snapshot_suffix => "smoke_async",
	}, {
		let args = Args::default_test(&["smoke", "fresh", "cleanup"]);
		let runtime = Runtime::new(Some(&args))?;
		let server = Server::new(Some(&args), Some(&runtime))?;
		let result = runtime.block_on(async {
			gaussmatrix::async_exec(&server).await
		});

		drop(runtime);
		assert_debug_snapshot!(result);
		result
	})
}
