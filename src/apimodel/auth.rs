//! Request authentication helpers.

/// Extract the bearer access token from a request.
///
/// Matrix accepts the access token in the `Authorization: Bearer <token>`
/// header (preferred) or, deprecated, the `access_token` query parameter. The
/// header takes precedence; a present-but-malformed `Authorization` header
/// yields no token (it does not fall back to the query parameter).
#[must_use]
pub fn extract_access_token(
	authorization: Option<&str>,
	query_token: Option<&str>,
) -> Option<String> {
	match authorization {
		| Some(header) => header
			.strip_prefix("Bearer ")
			.map(str::trim)
			.filter(|token| !token.is_empty())
			.map(ToOwned::to_owned),
		| None => query_token
			.filter(|token| !token.is_empty())
			.map(ToOwned::to_owned),
	}
}
