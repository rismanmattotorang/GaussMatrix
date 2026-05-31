use axum::{Json, extract::State, response::IntoResponse};
use serde_json::json;
use tuwunel_core::Result;

/// # `GET /_matrix/federation/v1/query/edutypes`
///
/// MSC4373: advertise which EDU types this server wishes to receive.
#[tracing::instrument(skip_all, level = "debug")]
pub(crate) async fn get_edu_types_route(
	State(services): State<crate::State>,
) -> Result<impl IntoResponse> {
	let cfg = &services.server.config;

	Ok(Json(json!({
		"m.presence": cfg.allow_incoming_presence,
		"m.receipt": cfg.allow_incoming_read_receipts,
		"m.typing": cfg.allow_incoming_typing,
	})))
}
