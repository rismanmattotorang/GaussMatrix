use std::sync::atomic::Ordering;

use gaussmatrix::{Server, args, restart, runtime::Runtime};
use gaussmatrix_core::{Result, debug_info};

fn main() -> Result {
	let args = args::parse();
	let runtime = Runtime::new(Some(&args))?;
	let server = Server::new(Some(&args), Some(&runtime))?;

	gaussmatrix::exec(&server, runtime)?;

	#[cfg(unix)]
	if server.server.restarting.load(Ordering::Acquire) {
		restart::restart();
	}

	debug_info!("Exit");
	Ok(())
}
