//! The `GET /_matrix/client/versions` response model.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

/// The response to `GET /_matrix/client/versions`: the supported Matrix
/// specification versions and the enabled unstable features.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Versions {
	/// Supported spec versions, e.g. `["v1.11", "v1.12"]`.
	pub versions: Vec<String>,
	/// Unstable features advertised to clients, keyed by feature flag.
	pub unstable_features: BTreeMap<String, bool>,
}

impl Versions {
	/// A response advertising the given supported spec versions.
	#[must_use]
	pub fn new(versions: &[&str]) -> Self {
		Self {
			versions: versions.iter().map(|v| (*v).to_owned()).collect(),
			unstable_features: BTreeMap::new(),
		}
	}

	/// Advertise an unstable feature flag.
	#[must_use]
	pub fn with_unstable_feature(mut self, name: &str, enabled: bool) -> Self {
		self.unstable_features.insert(name.to_owned(), enabled);
		self
	}

	/// The wire response body.
	#[must_use]
	pub fn to_json(&self) -> Value {
		let features: Map<String, Value> = self
			.unstable_features
			.iter()
			.map(|(name, enabled)| (name.clone(), Value::Bool(*enabled)))
			.collect();

		let mut body = Map::new();
		body.insert("versions".to_owned(), Value::from(self.versions.clone()));
		body.insert("unstable_features".to_owned(), Value::Object(features));

		Value::Object(body)
	}
}
