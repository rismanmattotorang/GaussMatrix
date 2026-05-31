use futures::{StreamExt, future::join};
use ruma::{api::client::sync::sync_events::v5::response, events::AnyRawAccountDataEvent};
use tuwunel_core::{
	Result, extract_variant,
	utils::{IterStream, ReadyExt, stream::BroadbandExt},
};
use tuwunel_service::sync::Room;

use super::{Connection, SyncInfo, Window, selector};
use crate::client::is_empty_account_data_event;

#[tracing::instrument(name = "account_data", level = "trace", skip_all)]
pub(super) async fn collect(
	sync_info: SyncInfo<'_>,
	conn: &Connection,
	window: &Window,
) -> Result<response::AccountData> {
	let SyncInfo { services, sender_user, .. } = sync_info;

	let implicit = conn
		.extensions
		.account_data
		.lists
		.as_deref()
		.map(<[_]>::iter);

	let explicit = conn
		.extensions
		.account_data
		.rooms
		.as_deref()
		.map(<[_]>::iter);

	let rooms = selector(sync_info, conn, window, implicit, explicit)
		.stream()
		.broad_filter_map(async |room_id| {
			let &Room { roomsince, .. } = conn.rooms.get(room_id)?;
			let changes: Vec<_> = services
				.account_data
				.changes_since(Some(room_id), sender_user, roomsince, Some(conn.next_batch))
				.ready_filter_map(|e| extract_variant!(e, AnyRawAccountDataEvent::Room))
				.ready_filter(move |e| roomsince != 0 || !is_empty_account_data_event(e))
				.collect()
				.await;

			changes
				.is_empty()
				.eq(&false)
				.then(move || (room_id.to_owned(), changes))
		})
		.collect();

	let globalsince = conn.globalsince;
	let global = services
		.account_data
		.changes_since(None, sender_user, globalsince, Some(conn.next_batch))
		.ready_filter_map(|e| extract_variant!(e, AnyRawAccountDataEvent::Global))
		.ready_filter(move |e| globalsince != 0 || !is_empty_account_data_event(e))
		.collect();

	let (global, rooms) = join(global, rooms).await;

	Ok(response::AccountData { global, rooms })
}
