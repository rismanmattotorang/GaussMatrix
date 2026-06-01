//! Endpoint resolution: matching an incoming `(method, path)` against the
//! registered [`Endpoint`]s.
//!
//! This is the dispatch core of the HTTP ingress, distinguishing an unknown
//! path (→ `404` / `M_UNRECOGNIZED`) from a known path reached with the wrong
//! method (→ `405`), so the [`MatrixError`](crate::MatrixError) surface can
//! report each correctly.

use crate::endpoint::{Endpoint, Method, PathParams, match_template};

/// The outcome of resolving a request against the router.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route<'router> {
	/// A matching endpoint and the captured path parameters.
	Matched {
		/// The matched endpoint descriptor.
		endpoint: &'router Endpoint,
		/// The captured path parameters.
		params: PathParams,
	},
	/// The path matched a registered endpoint, but not with this method.
	MethodNotAllowed,
	/// No registered endpoint matched the path.
	NotFound,
}

/// A set of [`Endpoint`]s an incoming request is resolved against.
#[derive(Clone, Debug, Default)]
pub struct Router {
	endpoints: Vec<Endpoint>,
}

impl Router {
	/// An empty router.
	#[must_use]
	pub fn new() -> Self { Self::default() }

	/// Register an endpoint.
	pub fn register(&mut self, endpoint: Endpoint) -> &mut Self {
		self.endpoints.push(endpoint);
		self
	}

	/// Resolve a request to a matching endpoint and its path parameters,
	/// distinguishing an unknown path from a known path with the wrong method.
	#[must_use]
	pub fn resolve(&self, method: Method, path: &str) -> Route<'_> {
		let mut path_matched = false;
		for endpoint in &self.endpoints {
			if let Some(params) = match_template(endpoint.path, path) {
				path_matched = true;
				if endpoint.method == method {
					return Route::Matched { endpoint, params };
				}
			}
		}

		if path_matched { Route::MethodNotAllowed } else { Route::NotFound }
	}
}
