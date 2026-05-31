use std::{
	net::{SocketAddr, TcpListener},
	path::Path,
};

use axum::{Router, extract::connect_info::IntoMakeServiceWithConnectInfo};
use axum_server::Handle;
use axum_server_dual_protocol::{ServerExt, axum_server::tls_rustls::RustlsConfig};
use futures::{FutureExt, future::BoxFuture};
use tuwunel_core::{Result, debug, err, info, itertools::Itertools};

pub(super) async fn serve<'a>(
	app: &Router,
	handle: &Handle<SocketAddr>,
	cert: &Path,
	key: &Path,
	dual_protocol: bool,
	listeners: impl Iterator<Item = TcpListener>,
	addrs: &[SocketAddr],
) -> Result<Vec<BoxFuture<'a, Result<(), std::io::Error>>>> {
	info!(
		"Note: It is strongly recommended that you use a reverse proxy instead of running \
		 tuwunel directly with TLS."
	);

	debug!(
		"Using direct TLS. Certificate path {cert:?} and certificate private key path {key:?}"
	);

	let conf = RustlsConfig::from_pem_file(cert, key)
		.await
		.map_err(|e| err!(Config("tls", "Failed to load certificates or key: {e}")))?;

	let app = app
		.clone()
		.into_make_service_with_connect_info::<SocketAddr>();

	if dual_protocol {
		serve_dual_protocol(&app, &conf, handle, listeners, addrs)
	} else {
		serve_tls(&app, &conf, handle, listeners, addrs)
	}
}

fn serve_dual_protocol<'a>(
	app: &IntoMakeServiceWithConnectInfo<Router, SocketAddr>,
	conf: &RustlsConfig,
	handle: &Handle<SocketAddr>,
	listeners: impl Iterator<Item = TcpListener>,
	addrs: &[SocketAddr],
) -> Result<Vec<BoxFuture<'a, Result<(), std::io::Error>>>> {
	let bound_servers = addrs.iter().map(|addr| -> Result<_> {
		Ok(axum_server_dual_protocol::bind_dual_protocol(*addr, conf.clone()))
	});

	let passed_servers = listeners.map(|listener| -> Result<_> {
		Ok(axum_server_dual_protocol::from_tcp_dual_protocol(
			listener.try_clone()?,
			conf.clone(),
		)?
		.set_upgrade(false))
	});

	bound_servers
		.chain(passed_servers)
		.map_ok(|server| {
			server
				.handle(handle.clone())
				.serve(app.clone())
				.boxed()
		})
		.collect()
}

fn serve_tls<'a>(
	app: &IntoMakeServiceWithConnectInfo<Router, SocketAddr>,
	conf: &RustlsConfig,
	handle: &Handle<SocketAddr>,
	listeners: impl Iterator<Item = TcpListener>,
	addrs: &[SocketAddr],
) -> Result<Vec<BoxFuture<'a, Result<(), std::io::Error>>>> {
	let bound_servers = addrs
		.iter()
		.map(|addr| -> Result<_> { Ok(axum_server::bind_rustls(*addr, conf.clone())) });

	let passed_servers = listeners.map(|listener| -> Result<_> {
		Ok(axum_server::from_tcp_rustls(listener.try_clone()?, conf.clone())?)
	});

	bound_servers
		.chain(passed_servers)
		.map_ok(|server| {
			server
				.handle(handle.clone())
				.serve(app.clone())
				.boxed()
		})
		.collect()
}
