use ruma::api::client::account::request_3pid_management_token_via_msisdn;
use gaussmatrix_core::{Err, Result};

use crate::Ruma;

/// # `POST /_matrix/client/v3/account/3pid/msisdn/requestToken`
///
/// "This API should be used to request validation tokens when adding an phone
/// number to an account"
///
/// - 403 signals that The homeserver does not allow the third party identifier
///   as a contact option.
pub(crate) async fn request_3pid_management_token_via_msisdn_route(
	_body: Ruma<request_3pid_management_token_via_msisdn::v3::Request>,
) -> Result<request_3pid_management_token_via_msisdn::v3::Response> {
	Err!(Request(ThreepidDenied("Third party identifiers are not implemented")))
}
