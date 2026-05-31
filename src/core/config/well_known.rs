use std::collections::BTreeMap;

use ruma::api::{
	client::{discovery::discover_support::Contact, rtc::RtcTransport},
	identity_service::tos::get_terms_of_service::v2::{LocalizedPolicy, Policies},
};
use tuwunel_macros::implement;

use crate::{Result, err, error::inspect_log};

#[implement(super::WellKnownConfig)]
pub fn get_contacts(&self) -> Vec<Contact> {
	let single_contact = self.support_role.clone().map(|role| Contact {
		role,
		email_address: self.support_email.clone(),
		matrix_id: self.support_mxid.clone(),
		pgp_key: self.support_pgp_key.clone(),
	});

	let contacts = self
		.support_contact
		.clone()
		.into_values()
		.map(Into::into);

	contacts.chain(single_contact).collect()
}

#[implement(super::WellKnownConfig)]
#[must_use]
pub fn get_policies(&self) -> BTreeMap<String, Policies> {
	self.support_policy
		.iter()
		.map(|(id, policy)| {
			let localized = policy
				.policy_translation
				.iter()
				.map(|(language, translation)| {
					(language.clone(), LocalizedPolicy::from(translation.clone()))
				})
				.collect();

			(id.clone(), Policies {
				version: policy.version.clone(),
				localized,
			})
		})
		.collect()
}

/// Build the configured RTC transports as `RtcTransport` values, the typed
/// form shared between `.well-known/matrix/client.rtc_foci` and the
/// `/rtc/transports` endpoint.
#[implement(super::WellKnownConfig)]
pub fn get_transports(&self) -> Result<Vec<RtcTransport>> {
	let custom = self.rtc_transports.iter().map(|item| {
		let mut data = item
			.as_object()
			.cloned()
			.ok_or_else(|| err!("`rtc_transport` is not a valid object"))?;

		let transport_type = data
			.remove("type")
			.and_then(|v| v.as_str().map(str::to_owned))
			.ok_or_else(|| err!("`type` is not a valid string"))?;

		RtcTransport::new(&transport_type, data).map_err(|e| {
			err!(Config("global.well_known.rtc_transports", "Malformed value(s): {e:?}"))
		})
	});

	let livekit_url = self
		.livekit_url
		.iter()
		.cloned()
		.map(|url| Ok(RtcTransport::livekit(url)));

	custom
		.chain(livekit_url)
		.collect::<Result<Vec<_>>>()
		.inspect_err(inspect_log)
}
