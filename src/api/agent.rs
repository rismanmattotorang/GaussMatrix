//! The agentic gateway endpoints (SPECS §IV-B).
//!
//! These are the HTTP channels through which an agent acts and reports back.
//! `POST /_gauss/agent/v1/rooms/{roomId}/mcp` is the action channel: an
//! authenticated, room-joined agent submits a Model Context Protocol JSON-RPC
//! request scoped to the room's capability grant — a `tools/call` is mediated,
//! audited, and reflected in-band, while `tools/list` / `resources/list` return
//! grant-scoped listings. `POST /_gauss/agent/v1/rooms/{roomId}/tool_result`
//! closes the loop: the agent runtime reports a completed call's result, which
//! is posted in-band as an `m.gauss.agent.tool_result`, correlated by `call_id`.
//! `POST /_gauss/agent/v1/rooms/{roomId}/approval` is the human-in-the-loop
//! gate: a human room member approves or rejects a call that required approval
//! (agent identities cannot approve), recorded in-band and in the audit log.
//! `PUT/GET/DELETE /_gauss/agent/v1/provision/{userId}` provisions, reads, and
//! revokes an agent through the Application Service API (§IV-A): an appservice
//! binds a cross-signing key to a user in its namespace, and only provisioned
//! agents may use the action endpoints. `GET /_gauss/agent/v1/rooms/{roomId}/grant`
//! lets a room member read the room's effective capability grant. Every action
//! is scoped, mediated, auditable, and visible in-band.

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
use ruma::{OwnedRoomId, OwnedUserId, RoomId, UserId};
use serde::Deserialize;
use serde_json::{Value, json};
use gaussmatrix_core::{Err, Result, err};
use gaussmatrix_service::appservice::RegistrationInfo;

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
	let agent = authenticate_agent(&services, auth.token(), &room_id).await?;

	match services.agent.handle_mcp_request(&agent, &room_id, &request).await? {
		| Some(response) => Ok(Json(response)),
		| None => Err!(Request(InvalidParam("Unsupported MCP method."))),
	}
}

/// The body of an agent provisioning request.
#[derive(Deserialize)]
pub(crate) struct ProvisionBody {
	/// The agent's bound cross-signing master public key (opaque, base64).
	signing_key: String,
	/// An optional human-readable display name.
	#[serde(default)]
	display_name: Option<String>,
}

/// `PUT /_gauss/agent/v1/provision/{userId}` — provision an agent identity.
///
/// Authenticated by an **Application Service** access token; the appservice may
/// only provision a user within its own declared namespace (§IV-A). Binds the
/// agent to a cross-signing public key and records it in the registry.
pub(crate) async fn provision_route(
	State(services): State<crate::State>,
	Path(user_id): Path<OwnedUserId>,
	TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
	Json(body): Json<ProvisionBody>,
) -> Result<impl IntoResponse> {
	let info = authorize_appservice(&services, auth.token(), &user_id).await?;

	let profile = services.agent.provision_agent(
		&user_id,
		&info.registration.id,
		&body.signing_key,
		body.display_name.as_deref(),
	)?;

	Ok(Json(json!({
		"agent_id": profile.agent_id,
		"operator": profile.operator,
		"display_name": profile.display_name,
	})))
}

/// `GET /_gauss/agent/v1/provision/{userId}` — read an agent's provisioning
/// record. Authenticated by an appservice token scoped to the agent's namespace.
pub(crate) async fn get_profile_route(
	State(services): State<crate::State>,
	Path(user_id): Path<OwnedUserId>,
	TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<impl IntoResponse> {
	authorize_appservice(&services, auth.token(), &user_id).await?;

	match services.agent.agent_profile(&user_id)? {
		| Some(profile) => Ok(Json(profile.to_content())),
		| None => Err!(Request(NotFound("No such provisioned agent."))),
	}
}

/// `DELETE /_gauss/agent/v1/provision/{userId}` — revoke an agent's provisioning.
/// Authenticated by an appservice token scoped to the agent's namespace.
pub(crate) async fn revoke_route(
	State(services): State<crate::State>,
	Path(user_id): Path<OwnedUserId>,
	TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<impl IntoResponse> {
	authorize_appservice(&services, auth.token(), &user_id).await?;

	let deprovisioned = services.agent.deprovision_agent(&user_id)?;

	Ok(Json(json!({ "deprovisioned": deprovisioned })))
}

/// Authenticate an appservice token and require that `user_id` is within the
/// appservice's declared namespace — the admission check for agent provisioning
/// management (§IV-A).
async fn authorize_appservice(
	services: &crate::State,
	token: &str,
	user_id: &UserId,
) -> Result<RegistrationInfo> {
	let info = services
		.appservice
		.find_from_access_token(token)
		.await
		.map_err(|_| err!(Request(Forbidden("A valid appservice token is required."))))?;

	if !info.is_user_match(user_id) {
		return Err!(Request(Forbidden("Agent id is outside the appservice's namespace.")));
	}

	Ok(info)
}

/// The body of a tool-call approval decision.
#[derive(Deserialize)]
pub(crate) struct ApprovalBody {
	/// The `call_id` of the tool call being decided.
	call_id: String,
	/// Whether the call is approved.
	approved: bool,
	/// An optional human-readable rationale.
	#[serde(default)]
	reason: Option<String>,
}

/// `POST /_gauss/agent/v1/rooms/{roomId}/approval` — human-in-the-loop decision.
///
/// A human room member approves or rejects a tool call that required approval.
/// Approval is human-in-the-loop by construction: agent identities (§IV-A)
/// cannot approve. The decision is audited and posted in-band as an
/// `m.gauss.agent.tool_approval`; the response carries the resulting event id.
pub(crate) async fn approval_route(
	State(services): State<crate::State>,
	Path(room_id): Path<OwnedRoomId>,
	TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
	Json(body): Json<ApprovalBody>,
) -> Result<impl IntoResponse> {
	let reviewer = authenticate_member(&services, auth.token(), &room_id).await?;

	if services.agent.is_agent(&reviewer) {
		return Err!(Request(Forbidden(
			"Tool-call approval is human-in-the-loop; agent identities cannot approve."
		)));
	}

	let event_id = services
		.agent
		.record_approval(&reviewer, &room_id, &body.call_id, body.approved, body.reason.as_deref())
		.await?;

	Ok(Json(json!({ "event_id": event_id })))
}

/// `POST /_gauss/agent/v1/rooms/{roomId}/tool_result` — report a tool result.
///
/// The agent runtime reports a completed call's result (`call_id` plus `output`
/// or `error`); it is posted in-band as an `m.gauss.agent.tool_result`. The
/// response carries the resulting event id.
pub(crate) async fn tool_result_route(
	State(services): State<crate::State>,
	Path(room_id): Path<OwnedRoomId>,
	TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
	Json(content): Json<Value>,
) -> Result<impl IntoResponse> {
	let agent = authenticate_agent(&services, auth.token(), &room_id).await?;

	let event_id = services.agent.ingest_tool_result(&agent, &room_id, &content).await?;

	Ok(Json(json!({ "event_id": event_id })))
}

/// `GET /_gauss/agent/v1/rooms/{roomId}/grant` — read a room's effective
/// capability grant. Any joined room member may inspect it.
pub(crate) async fn get_grant_route(
	State(services): State<crate::State>,
	Path(room_id): Path<OwnedRoomId>,
	TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<impl IntoResponse> {
	authenticate_member(&services, auth.token(), &room_id).await?;

	let grant = services.agent.grant_for(&room_id).await;

	Ok(Json(grant.to_content()))
}

/// Authenticate a calling **agent**: a room member (per [`authenticate_member`])
/// that has been provisioned through the registry (§IV-A). The action endpoints
/// are reachable only by provisioned agents.
async fn authenticate_agent(
	services: &crate::State,
	token: &str,
	room_id: &RoomId,
) -> Result<OwnedUserId> {
	let agent = authenticate_member(services, token, room_id).await?;

	if !services.agent.is_provisioned(&agent)? {
		return Err!(Request(Forbidden("Caller is not a provisioned agent.")));
	}

	Ok(agent)
}

/// Authenticate the caller from its access `token` and require that it is joined
/// to `room_id` — the shared admission check for the gateway endpoints.
async fn authenticate_member(
	services: &crate::State,
	token: &str,
	room_id: &RoomId,
) -> Result<OwnedUserId> {
	let (agent, _device, expires_at) = services
		.users
		.find_from_token(token)
		.await
		.map_err(|_| err!(Request(MissingToken("Invalid access token."))))?;

	if expires_at.is_some_and(|expiry| expiry <= SystemTime::now()) {
		return Err!(Request(MissingToken("Access token has expired.")));
	}

	if !services.state_cache.is_joined(&agent, room_id).await {
		return Err!(Request(Forbidden("Agent is not joined to the target room.")));
	}

	Ok(agent)
}
