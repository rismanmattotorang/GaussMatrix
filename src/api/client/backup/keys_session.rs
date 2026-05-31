use std::cmp::Ordering;

use axum::extract::State;
use ruma::{
	UInt,
	api::client::backup::{
		add_backup_keys_for_session, delete_backup_keys_for_session, get_backup_keys_for_session,
	},
};
use gaussmatrix_core::{Err, Result, err};

use super::get_count_etag;
use crate::Ruma;

/// # `PUT /_matrix/client/r0/room_keys/keys/{roomId}/{sessionId}`
///
/// Add the received backup key to the database.
///
/// - Only manipulating the most recently created version of the backup is
///   allowed
/// - Adds the keys to the backup
/// - Returns the new number of keys in this backup and the etag
pub(crate) async fn add_backup_keys_for_session_route(
	State(services): State<crate::State>,
	body: Ruma<add_backup_keys_for_session::v3::Request>,
) -> Result<add_backup_keys_for_session::v3::Response> {
	if services
		.key_backups
		.get_latest_backup_version(body.sender_user())
		.await
		.is_ok_and(|version| version != body.version)
	{
		return Err!(Request(InvalidParam(
			"You may only manipulate the most recently created version of the backup."
		)));
	}

	// Check if we already have a better key
	let mut ok_to_replace = true;
	if let Some(old_key) = &services
		.key_backups
		.get_session(body.sender_user(), &body.version, &body.room_id, &body.session_id)
		.await
		.ok()
	{
		let old_is_verified = old_key
			.get_field::<bool>("is_verified")?
			.unwrap_or_default();

		let new_is_verified = body
			.session_data
			.get_field::<bool>("is_verified")?
			.ok_or_else(|| err!(Request(BadJson("`is_verified` field should exist"))))?;

		// Prefer key that `is_verified`
		if old_is_verified != new_is_verified {
			if old_is_verified {
				ok_to_replace = false;
			}
		} else {
			// If both have same `is_verified`, prefer the one with lower
			// `first_message_index`
			let old_first_message_index = old_key
				.get_field::<UInt>("first_message_index")?
				.unwrap_or(UInt::MAX);

			let new_first_message_index = body
				.session_data
				.get_field::<UInt>("first_message_index")?
				.ok_or_else(|| {
					err!(Request(BadJson("`first_message_index` field should exist")))
				})?;

			ok_to_replace = match new_first_message_index.cmp(&old_first_message_index) {
				| Ordering::Less => true,
				| Ordering::Greater => false,
				| Ordering::Equal => {
					// If both have same `first_message_index`, prefer the one with lower
					// `forwarded_count`
					let old_forwarded_count = old_key
						.get_field::<UInt>("forwarded_count")?
						.unwrap_or(UInt::MAX);

					let new_forwarded_count = body
						.session_data
						.get_field::<UInt>("forwarded_count")?
						.ok_or_else(|| {
							err!(Request(BadJson("`forwarded_count` field should exist")))
						})?;

					new_forwarded_count < old_forwarded_count
				},
			};
		}
	}

	if ok_to_replace {
		services
			.key_backups
			.add_key(
				body.sender_user(),
				&body.version,
				&body.room_id,
				&body.session_id,
				&body.session_data,
			)
			.await?;
	}

	let (count, etag) = get_count_etag(&services, body.sender_user(), &body.version).await?;

	Ok(add_backup_keys_for_session::v3::Response { count, etag })
}

/// # `GET /_matrix/client/r0/room_keys/keys/{roomId}/{sessionId}`
///
/// Retrieves a key from the backup.
pub(crate) async fn get_backup_keys_for_session_route(
	State(services): State<crate::State>,
	body: Ruma<get_backup_keys_for_session::v3::Request>,
) -> Result<get_backup_keys_for_session::v3::Response> {
	let key_data = services
		.key_backups
		.get_session(body.sender_user(), &body.version, &body.room_id, &body.session_id)
		.await
		.map_err(|_| {
			err!(Request(NotFound(debug_error!("Backup key not found for this user's session."))))
		})?;

	Ok(get_backup_keys_for_session::v3::Response { key_data })
}

/// # `DELETE /_matrix/client/r0/room_keys/keys/{roomId}/{sessionId}`
///
/// Delete a key from the backup.
pub(crate) async fn delete_backup_keys_for_session_route(
	State(services): State<crate::State>,
	body: Ruma<delete_backup_keys_for_session::v3::Request>,
) -> Result<delete_backup_keys_for_session::v3::Response> {
	services
		.key_backups
		.delete_room_key(body.sender_user(), &body.version, &body.room_id, &body.session_id)
		.await;

	let (count, etag) = get_count_etag(&services, body.sender_user(), &body.version).await?;

	Ok(delete_backup_keys_for_session::v3::Response { count, etag })
}
