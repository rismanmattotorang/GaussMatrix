#![cfg(test)]

use insta::{assert_debug_snapshot, with_settings};
use gaussmatrix::{Args, Runtime, Server};
use gaussmatrix_core::Result;

#[test]
fn dummy() {}

#[test]
#[should_panic = "dummy"]
fn panic_dummy() { panic!("dummy") }

#[test]
fn smoke() -> Result {
	with_settings!({
		description => "Smoke Test",
		snapshot_suffix => "smoke_test",
	}, {
		let args = Args::default_test(&["smoke", "fresh", "cleanup"]);
		let runtime = Runtime::new(Some(&args))?;
		let server = Server::new(Some(&args), Some(&runtime))?;
		let result = gaussmatrix::exec(&server, runtime);

		assert_debug_snapshot!(result);
		result
	})
}
