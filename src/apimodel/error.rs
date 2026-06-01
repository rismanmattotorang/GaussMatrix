//! The standard Matrix API error model.
//!
//! Every Client–Server and Server–Server endpoint reports failures with a
//! standard body — `{ "errcode": "M_…", "error": "…" }`, plus `retry_after_ms`
//! for rate limits — and a corresponding HTTP status. This module models that
//! error contract so the typed request/response surface built on top of it can
//! produce conformant responses.

use serde_json::{Map, Value};

/// A standard Matrix error code (the `errcode` field).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorCode {
	/// The request was forbidden by the server's authorisation rules.
	Forbidden,
	/// The access token was not recognised.
	UnknownToken,
	/// No access token was supplied.
	MissingToken,
	/// Authorisation is required and has failed or not been provided.
	Unauthorized,
	/// The request body was not valid JSON for the endpoint.
	BadJson,
	/// The request body was not JSON at all.
	NotJson,
	/// The requested resource was not found.
	NotFound,
	/// The endpoint or method is not recognised by the server.
	Unrecognized,
	/// Too many requests have been sent; see `retry_after_ms`.
	LimitExceeded,
	/// The desired user id is already taken.
	UserInUse,
	/// The desired user id is not a valid user name.
	InvalidUsername,
	/// The desired room alias is already taken.
	RoomInUse,
	/// The room version is not supported by the server.
	UnsupportedRoomVersion,
	/// A required request parameter was missing.
	MissingParam,
	/// A request parameter was invalid.
	InvalidParam,
	/// The request or an attached resource was too large.
	TooLarge,
	/// An otherwise-unclassified error.
	Unknown,
}

impl ErrorCode {
	/// The wire `errcode` string (e.g. `M_FORBIDDEN`).
	#[must_use]
	pub const fn errcode(self) -> &'static str {
		match self {
			| Self::Forbidden => "M_FORBIDDEN",
			| Self::UnknownToken => "M_UNKNOWN_TOKEN",
			| Self::MissingToken => "M_MISSING_TOKEN",
			| Self::Unauthorized => "M_UNAUTHORIZED",
			| Self::BadJson => "M_BAD_JSON",
			| Self::NotJson => "M_NOT_JSON",
			| Self::NotFound => "M_NOT_FOUND",
			| Self::Unrecognized => "M_UNRECOGNIZED",
			| Self::LimitExceeded => "M_LIMIT_EXCEEDED",
			| Self::UserInUse => "M_USER_IN_USE",
			| Self::InvalidUsername => "M_INVALID_USERNAME",
			| Self::RoomInUse => "M_ROOM_IN_USE",
			| Self::UnsupportedRoomVersion => "M_UNSUPPORTED_ROOM_VERSION",
			| Self::MissingParam => "M_MISSING_PARAM",
			| Self::InvalidParam => "M_INVALID_PARAM",
			| Self::TooLarge => "M_TOO_LARGE",
			| Self::Unknown => "M_UNKNOWN",
		}
	}

	/// The HTTP status code conventionally returned with this error.
	#[must_use]
	pub const fn http_status(self) -> u16 {
		match self {
			| Self::Unauthorized | Self::UnknownToken | Self::MissingToken => 401,
			| Self::Forbidden => 403,
			| Self::NotFound | Self::Unrecognized => 404,
			| Self::TooLarge => 413,
			| Self::LimitExceeded => 429,
			| Self::Unknown => 500,
			// The remaining client errors are 400 Bad Request.
			| Self::BadJson
			| Self::NotJson
			| Self::UserInUse
			| Self::InvalidUsername
			| Self::RoomInUse
			| Self::UnsupportedRoomVersion
			| Self::MissingParam
			| Self::InvalidParam => 400,
		}
	}
}

/// A Matrix API error: an [`ErrorCode`], a human-readable message, and an
/// optional `retry_after_ms` for rate limiting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatrixError {
	/// The error code.
	pub code: ErrorCode,
	/// The human-readable `error` message.
	pub message: String,
	/// Milliseconds to wait before retrying, for [`ErrorCode::LimitExceeded`].
	pub retry_after_ms: Option<u64>,
}

impl MatrixError {
	/// A new error with the given code and message.
	#[must_use]
	pub fn new(code: ErrorCode, message: &str) -> Self {
		Self { code, message: message.to_owned(), retry_after_ms: None }
	}

	/// A rate-limit error carrying the retry delay.
	#[must_use]
	pub fn rate_limited(message: &str, retry_after_ms: u64) -> Self {
		Self {
			code: ErrorCode::LimitExceeded,
			message: message.to_owned(),
			retry_after_ms: Some(retry_after_ms),
		}
	}

	/// The HTTP status code for this error.
	#[must_use]
	pub const fn http_status(&self) -> u16 { self.code.http_status() }

	/// The wire response body: `{ "errcode", "error", [retry_after_ms] }`.
	#[must_use]
	pub fn to_json(&self) -> Value {
		let mut body = Map::new();
		body.insert("errcode".to_owned(), Value::from(self.code.errcode()));
		body.insert("error".to_owned(), Value::from(self.message.clone()));
		if let Some(retry_after_ms) = self.retry_after_ms {
			body.insert("retry_after_ms".to_owned(), Value::from(retry_after_ms));
		}

		Value::Object(body)
	}
}

impl std::fmt::Display for MatrixError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}: {}", self.code.errcode(), self.message)
	}
}

impl std::error::Error for MatrixError {}
