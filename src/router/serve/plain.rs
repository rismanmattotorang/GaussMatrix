use std::net::{SocketAddr, TcpListener};

use axum::Router;
use axum_server::Handle;
use futures::{FutureExt, future::BoxFuture};
use tuwunel_core::{Result, itertools::Itertools};

pub(super) fn serve<'a>(
	router: &Router,
	handle: &Handle<SocketAddr>,
	listeners: impl Iterator<Item = TcpListener>,
	addrs: &[SocketAddr],
) -> Result<Vec<BoxFuture<'a, Result<(), std::io::Error>>>> {
	let router = router
		.clone()
		.into_make_service_with_connect_info::<SocketAddr>();

	let bound_servers = addrs
		.iter()
		.map(|addr| -> Result<_> { Ok(axum_server::bind(*addr)) });

	let passed_servers = listeners.map(|listener| Ok(axum_server::from_tcp(listener)?));

	let acceptors = bound_servers
		.chain(passed_servers)
		.map_ok(|server| {
			server
				.handle(handle.clone())
				.serve(router.clone())
				.boxed()
		})
		.collect::<Result<Vec<_>>>()?;

	Ok(acceptors)
}
