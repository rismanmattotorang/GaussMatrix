use serde::Deserialize;

/// Deserialization target for the upstream provider's `/token` JSON response.
/// Kept distinct from `Session` because some providers emit `expires_at` as a
/// Unix-timestamp integer, which would not deserialize into the
/// `SystemTime`-typed `Session::expires_at`.
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
	/// Token type (bearer, mac, etc).
	pub token_type: Option<String>,

	/// Access token granted by the provider.
	pub access_token: Option<String>,

	/// Duration in seconds the access_token is valid for.
	pub expires_in: Option<u64>,

	/// Token used to refresh the access_token.
	pub refresh_token: Option<String>,

	/// Duration in seconds the refresh_token is valid for.
	pub refresh_token_expires_in: Option<u64>,

	/// Access scope actually granted (if supported).
	pub scope: Option<String>,

	/// Signed JWT containing the user's identity claims (OIDC).
	pub id_token: Option<String>,
}
