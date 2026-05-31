use axum::extract::State;
use futures::{
	StreamExt,
	future::{join, join5},
};
use rand::seq::SliceRandom;
use ruma::{
	OwnedServerName,
	api::federation::query::{
		get_profile_information::{self, v1::Response as ProfileResponse},
		get_room_information,
	},
	profile::{ProfileFieldName, ProfileFieldValue},
};
use serde_json::Value as JsonValue;
use gaussmatrix_core::{Err, Result, err, utils::future::TryExtExt};

use crate::Ruma;

/// # `GET /_matrix/federation/v1/query/directory`
///
/// Resolve a room alias to a room id.
pub(crate) async fn get_room_information_route(
	State(services): State<crate::State>,
	body: Ruma<get_room_information::v1::Request>,
) -> Result<get_room_information::v1::Response> {
	let room_id = services
		.alias
		.resolve_local_alias(&body.room_alias)
		.await
		.map_err(|_| err!(Request(NotFound("Room alias not found."))))?;

	let mut servers: Vec<OwnedServerName> = services
		.state_cache
		.room_servers(&room_id)
		.map(ToOwned::to_owned)
		.collect()
		.await;

	servers.sort_unstable();
	servers.dedup();

	servers.shuffle(&mut rand::rng());

	// insert our server as the very first choice if in list
	if let Some(server_index) = servers
		.iter()
		.position(|server| server == services.globals.server_name())
	{
		servers.swap_remove(server_index);
		servers.insert(0, services.globals.server_name().to_owned());
	}

	Ok(get_room_information::v1::Response { room_id, servers })
}

/// # `GET /_matrix/federation/v1/query/profile`
///
///
/// Gets information on a profile.
pub(crate) async fn get_profile_information_route(
	State(services): State<crate::State>,
	body: Ruma<get_profile_information::v1::Request>,
) -> Result<ProfileResponse> {
	if !services
		.server
		.config
		.allow_inbound_profile_lookup_federation_requests
	{
		return Err!(Request(Forbidden(
			"Profile lookup over federation is not allowed on this homeserver.",
		)));
	}

	if !services.globals.user_is_local(&body.user_id) {
		return Err!(Request(InvalidParam("User does not belong to this server.",)));
	}

	match &body.field {
		| Some(ProfileFieldName::AvatarUrl | ProfileFieldName::DisplayName) => {
			let avatar_url = services.users.avatar_url(&body.user_id).ok();
			let displayname = services.users.displayname(&body.user_id).ok();
			let (avatar_url, displayname) = join(avatar_url, displayname).await;

			Ok([
				avatar_url.map(ProfileFieldValue::AvatarUrl),
				displayname.map(ProfileFieldValue::DisplayName),
			]
			.into_iter()
			.flatten()
			.collect())
		},
		| Some(custom_field) => {
			let value = services
				.users
				.profile_key(&body.user_id, custom_field.as_str())
				.await
				.ok();

			let entry = value
				.as_ref()
				.map(|raw| serde_json::to_value(raw.json()))
				.transpose()?
				.map(|json| (custom_field.as_str().to_owned(), json));

			Ok(entry.into_iter().collect())
		},
		| None => {
			let avatar_url = services.users.avatar_url(&body.user_id).ok();
			let blurhash = services.users.blurhash(&body.user_id).ok();
			let displayname = services.users.displayname(&body.user_id).ok();
			let tz = services.users.timezone(&body.user_id).ok();
			let custom = services
				.users
				.all_profile_keys(&body.user_id)
				.collect::<Vec<_>>();

			let (avatar_url, blurhash, custom, displayname, tz) =
				join5(avatar_url, blurhash, custom, displayname, tz).await;

			let mut response: ProfileResponse = [
				avatar_url.map(ProfileFieldValue::AvatarUrl),
				displayname.map(ProfileFieldValue::DisplayName),
				tz.map(ProfileFieldValue::TimeZone),
			]
			.into_iter()
			.flatten()
			.collect();

			if let Some(blurhash) = blurhash {
				response.set("blurhash".to_owned(), JsonValue::String(blurhash));
			}

			response.extend(
				custom
					.into_iter()
					.filter_map(|(k, v)| serde_json::to_value(v).ok().map(|v| (k, v))),
			);

			Ok(response)
		},
	}
}
