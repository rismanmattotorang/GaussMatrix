use axum::extract::State;
use ruma::api::client::admin::get_nonce;
use gaussmatrix_core::{Result, err};

use crate::Ruma;

/// # `GET /_synapse/admin/v1/register`
///
/// Issues a short-lived nonce. Returns 404 when the shared secret is not set.
pub(crate) async fn admin_register_nonce_route(
	State(services): State<crate::State>,
	_body: Ruma<get_nonce::v1::Request>,
) -> Result<get_nonce::v1::Response> {
	services
		.admin
		.register_is_enabled()
		.then(|| services.admin.issue_register_nonce())
		.map(get_nonce::v1::Response::new)
		.ok_or_else(|| err!(Request(Unknown("Shared-secret registration is not enabled"))))
}
