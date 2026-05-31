mod change_password;
mod deactivate;
mod request_3pid_management_token_via_email;
mod request_3pid_management_token_via_msisdn;
mod third_party;
mod whoami;

pub(crate) use self::{
	change_password::change_password_route, deactivate::deactivate_route,
	request_3pid_management_token_via_email::request_3pid_management_token_via_email_route,
	request_3pid_management_token_via_msisdn::request_3pid_management_token_via_msisdn_route,
	third_party::third_party_route, whoami::whoami_route,
};
