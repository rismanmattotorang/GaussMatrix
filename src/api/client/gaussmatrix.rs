use axum::{Json, extract::State, response::IntoResponse};
use futures::StreamExt;
use gaussmatrix_core::Result;

/// # `GET /_gaussmatrix/server_version`
///
/// GaussMatrix-specific API to get the server version, results akin to
/// `/_matrix/federation/v1/version`
pub(crate) async fn gaussmatrix_server_version() -> Result<impl IntoResponse> {
	Ok(Json(serde_json::json!({
		"name": gaussmatrix_core::version::name(),
		"version": gaussmatrix_core::version::version(),
	})))
}

/// # `GET /_gaussmatrix/local_user_count`
///
/// GaussMatrix-specific API to return the amount of users registered on this
/// homeserver. Endpoint is disabled if federation is disabled for privacy. This
/// only includes active users (not deactivated, no guests, etc)
pub(crate) async fn gaussmatrix_local_user_count(
	State(services): State<crate::State>,
) -> Result<impl IntoResponse> {
	let user_count = services.users.list_local_users().count().await;

	Ok(Json(serde_json::json!({
		"count": user_count
	})))
}
