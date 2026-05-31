use ruma::api::client::account::get_3pids;
use gaussmatrix_core::Result;

use crate::Ruma;

/// # `GET _matrix/client/v3/account/3pid`
///
/// Get a list of third party identifiers associated with this account.
///
/// - Currently always returns empty list
pub(crate) async fn third_party_route(
	body: Ruma<get_3pids::v3::Request>,
) -> Result<get_3pids::v3::Response> {
	let _sender_user = body
		.sender_user
		.as_ref()
		.expect("user is authenticated");

	Ok(get_3pids::v3::Response::new(Vec::new()))
}
