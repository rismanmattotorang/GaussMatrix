use axum::extract::State;
use ruma::api::client::config::delete_global_account_data;
use gaussmatrix_core::{Err, Result};

use crate::Ruma;

/// # `DELETE /_matrix/client/unstable/org.matrix.msc3391/user/{userId}/account_data/{type}`
///
/// MSC3391: erase the named global account data type for the user.
pub(crate) async fn delete_global_account_data_route(
	State(services): State<crate::State>,
	body: Ruma<delete_global_account_data::unstable::Request>,
) -> Result<delete_global_account_data::unstable::Response> {
	let sender_user = body.sender_user();

	if sender_user != body.user_id && body.appservice_info.is_none() {
		return Err!(Request(Forbidden("You cannot delete account data for other users.")));
	}

	services
		.account_data
		.delete(None, &body.user_id, body.event_type.to_string().into())
		.await?;

	Ok(delete_global_account_data::unstable::Response {})
}
