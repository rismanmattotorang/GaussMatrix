//! The agentic MCP gateway endpoint (SPECS §IV-B).
//!
//! `POST /_gauss/agent/v1/rooms/{roomId}/mcp` is the sole HTTP channel through
//! which an agent acts. An authenticated agent submits a Model Context Protocol
//! JSON-RPC request, scoped to the target room's capability grant: a
//! `tools/call` is mediated, recorded in the tamper-evident audit log, and — when
//! it proceeds — reflected in-band as an `m.gauss.agent.tool_call` event; the
//! read-only `tools/list` / `resources/list` methods return grant-scoped
//! listings. Every call is scoped, mediated, auditable, and visible in-band.

use std::time::SystemTime;

use axum::{
	Json,
	extract::{Path, State},
	response::IntoResponse,
};
use axum_extra::{
	TypedHeader,
	headers::{Authorization, authorization::Bearer},
};
use ruma::OwnedRoomId;
use serde_json::Value;
use gaussmatrix_core::{Err, Result, err};

/// `POST /_gauss/agent/v1/rooms/{roomId}/mcp` — the MCP gateway.
///
/// The bearer access token identifies the calling agent; the agent must be
/// joined to the target room. The request body is an MCP JSON-RPC request and
/// the response is the corresponding JSON-RPC reply.
pub(crate) async fn mcp_gateway_route(
	State(services): State<crate::State>,
	Path(room_id): Path<OwnedRoomId>,
	TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
	Json(request): Json<Value>,
) -> Result<impl IntoResponse> {
	let (agent, _device, expires_at) = services
		.users
		.find_from_token(auth.token())
		.await
		.map_err(|_| err!(Request(MissingToken("Invalid access token."))))?;

	if expires_at.is_some_and(|expiry| expiry <= SystemTime::now()) {
		return Err!(Request(MissingToken("Access token has expired.")));
	}

	if !services.state_cache.is_joined(&agent, &room_id).await {
		return Err!(Request(Forbidden("Agent is not joined to the target room.")));
	}

	match services.agent.handle_mcp_request(&agent, &room_id, &request).await? {
		| Some(response) => Ok(Json(response)),
		| None => Err!(Request(InvalidParam("Unsupported MCP method."))),
	}
}
