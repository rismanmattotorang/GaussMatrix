//! A typed model of Matrix API endpoints: method, path template, and the
//! authentication an endpoint requires, plus path-template matching.
//!
//! The Client–Server and Server–Server APIs are a set of `(method, path)` routes
//! whose paths carry parameters (e.g. `/_matrix/client/v3/rooms/{roomId}/state`).
//! This models a route as an [`Endpoint`] and matches a concrete request path
//! against its template, extracting the path parameters — what the HTTP ingress
//! dispatches on, pairing with the [`MatrixError`](crate::MatrixError) surface
//! (an unmatched route is `M_UNRECOGNIZED`).

use std::collections::BTreeMap;

/// An HTTP method used by a Matrix endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
	/// `GET`.
	Get,
	/// `PUT`.
	Put,
	/// `POST`.
	Post,
	/// `DELETE`.
	Delete,
}

/// The authentication an endpoint requires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthScope {
	/// No authentication (e.g. `/_matrix/client/versions`).
	None,
	/// A user access token.
	User,
	/// An application-service token.
	AppService,
	/// A signed federation (Server–Server) request.
	Server,
}

/// The captured path parameters of a matched route.
pub type PathParams = BTreeMap<String, String>;

/// A typed endpoint descriptor: an HTTP method, a path template with `{param}`
/// segments, and the authentication it requires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Endpoint {
	/// The HTTP method.
	pub method: Method,
	/// The path template, with `{param}` segments (e.g.
	/// `/_matrix/client/v3/rooms/{roomId}/state`).
	pub path: &'static str,
	/// The authentication the endpoint requires.
	pub auth: AuthScope,
}

impl Endpoint {
	/// A new endpoint descriptor.
	#[must_use]
	pub const fn new(method: Method, path: &'static str, auth: AuthScope) -> Self {
		Self { method, path, auth }
	}

	/// Match a concrete request against this endpoint, returning the captured
	/// path parameters when both the method and the path template match.
	#[must_use]
	pub fn matches(&self, method: Method, path: &str) -> Option<PathParams> {
		if method != self.method {
			return None;
		}

		match_template(self.path, path)
	}
}

/// Match a concrete `path` against a `{param}` `template`, returning the
/// captured parameters, or `None` when the path does not match.
///
/// A template segment of the form `{name}` matches any single path segment and
/// captures it under `name`; every other segment must match literally, and the
/// two must have the same number of segments.
#[must_use]
pub fn match_template(template: &str, path: &str) -> Option<PathParams> {
	let mut template_segments = template.split('/');
	let mut path_segments = path.split('/');
	let mut params = PathParams::new();

	loop {
		match (template_segments.next(), path_segments.next()) {
			| (Some(template_segment), Some(path_segment)) => {
				if let Some(name) = template_segment
					.strip_prefix('{')
					.and_then(|inner| inner.strip_suffix('}'))
				{
					params.insert(name.to_owned(), path_segment.to_owned());
				} else if template_segment != path_segment {
					return None;
				}
			},
			| (None, None) => return Some(params),
			// Differing segment counts.
			| _ => return None,
		}
	}
}
