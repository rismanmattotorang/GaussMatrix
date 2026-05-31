#![cfg(test)]

use axum::Extension;
use ipnet::IpNet;
use tower::util::Either;
use gaussmatrix_api::router::{ConfiguredIpSource, TrustedPeerSubnets};
use gaussmatrix_core::config::IpSource;

use super::{ip_source_layer, trusted_peer_subnets_layer};

#[test]
fn ip_source_layer_none_returns_identity_branch() {
	let layer = ip_source_layer(None);

	assert!(matches!(layer, Either::Right(_)));
}

#[test]
fn ip_source_layer_connect_info_returns_extension_branch() {
	let layer = ip_source_layer(Some(IpSource::ConnectInfo));

	assert!(matches!(layer, Either::Left(Extension(ConfiguredIpSource(_)))));
}

#[test]
fn trusted_peer_subnets_layer_empty_returns_identity_branch() {
	let layer = trusted_peer_subnets_layer(&[]);

	assert!(matches!(layer, Either::Right(_)));
}

#[test]
fn trusted_peer_subnets_layer_populated_returns_extension_branch() {
	let subnets: Vec<IpNet> =
		vec!["172.18.0.0/16".parse().expect("CIDR"), "fd00::/8".parse().expect("CIDR")];

	let layer = trusted_peer_subnets_layer(&subnets);

	let nets = match layer {
		| Either::Left(Extension(TrustedPeerSubnets(nets))) => nets,
		| Either::Right(_) => panic!("expected extension branch"),
	};

	assert_eq!(nets.len(), 2);
}
