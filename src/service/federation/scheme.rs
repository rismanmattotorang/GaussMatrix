//! Adapters that build the `Input` for ruma's [`AuthScheme`] and
//! [`PathBuilder`] traits from tuwunel's federation context.
//!
//! Federation endpoints span several auth/path-builder combinations
//! (`ServerSignatures` + `VersionHistory`, `ServerSignatures` + `SinglePath`,
//! and a handful of `NoAuthentication`/`NoAccessToken` variants). The
//! generic-associated-type `Input<'a>` of each ruma trait varies per impl, so
//! a single bound on `OutgoingRequest` cannot supply the right value uniformly.
//! [`FedAuth`] and [`FedPath`] each accept tuwunel's federation context and
//! return the appropriate `Input` for the concrete auth scheme or path builder
//! at the call site.

use std::borrow::Cow;

use ruma::{
	OwnedServerName,
	api::{
		SupportedVersions,
		auth_scheme::{AuthScheme, NoAccessToken, NoAuthentication, SendAccessToken},
		federation::authentication::{ServerSignatures, ServerSignaturesInput},
		path_builder::{PathBuilder, SinglePath, VersionHistory},
	},
	signatures::Ed25519KeyPair,
};

pub trait FedAuth: AuthScheme {
	fn input<'a>(
		origin: OwnedServerName,
		dest: OwnedServerName,
		keypair: &'a Ed25519KeyPair,
	) -> <Self as AuthScheme>::Input<'a>;
}

impl FedAuth for NoAuthentication {
	fn input(_: OwnedServerName, _: OwnedServerName, _: &Ed25519KeyPair) {}
}

impl FedAuth for NoAccessToken {
	fn input<'a>(
		_: OwnedServerName,
		_: OwnedServerName,
		_: &'a Ed25519KeyPair,
	) -> SendAccessToken<'a> {
		SendAccessToken::None
	}
}

impl FedAuth for ServerSignatures {
	fn input<'a>(
		origin: OwnedServerName,
		dest: OwnedServerName,
		keypair: &'a Ed25519KeyPair,
	) -> ServerSignaturesInput<'a> {
		ServerSignaturesInput::new(origin, dest, keypair)
	}
}

pub trait FedPath: PathBuilder {
	fn input<'a>(supported: &'a SupportedVersions) -> <Self as PathBuilder>::Input<'a>;
}

impl FedPath for SinglePath {
	fn input(_: &SupportedVersions) {}
}

impl FedPath for VersionHistory {
	fn input<'a>(supported: &'a SupportedVersions) -> Cow<'a, SupportedVersions> {
		Cow::Borrowed(supported)
	}
}
