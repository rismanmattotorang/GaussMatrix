use axum::extract::State;
use ruma::api::client::reporting::report_user;
use tuwunel_core::{Err, Result, info};

use super::REASON_MAX_LEN;
use crate::{ClientIp, Ruma};

/// # `POST /_matrix/client/v3/users/{userId}/report`
///
/// Reports an inappropriate user to homeserver admins (MSC4260).
#[tracing::instrument(skip_all, fields(%client), name = "report_user")]
pub(crate) async fn report_user_route(
	State(services): State<crate::State>,
	ClientIp(client): ClientIp,
	body: Ruma<report_user::v3::Request>,
) -> Result<report_user::v3::Response> {
	let sender_user = body.sender_user();

	if body.reason.len().gt(&REASON_MAX_LEN) {
		return Err!(Request(InvalidParam(
			"Reason too long, should be {REASON_MAX_LEN} characters or fewer"
		)));
	}

	if !services
		.users
		.is_active_local(&body.user_id)
		.await
	{
		return Err!(Request(NotFound("User does not exist")));
	}

	info!(
		"Received user report by user {sender_user} for user {} with reason: \"{}\"",
		body.user_id, body.reason,
	);

	services
		.admin
		.send_text(&format!(
			"@room User report received from {}\nReport Reason: {}\n\nReported User ID: {}",
			sender_user, body.reason, body.user_id,
		))
		.await;

	Ok(report_user::v3::Response {})
}
