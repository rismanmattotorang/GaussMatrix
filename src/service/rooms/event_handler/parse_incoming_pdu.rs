use ruma::{CanonicalJsonObject, CanonicalJsonValue, OwnedEventId, OwnedRoomId};
use serde_json::value::RawValue as RawJsonValue;
use tuwunel_core::{Result, err, implement, matrix::event::gen_event_id, result::FlatOk};

type Parsed = (OwnedRoomId, OwnedEventId, CanonicalJsonObject);

#[implement(super::Service)]
#[tracing::instrument(
    name = "parse_incoming",
    level = "trace",
    skip_all,
    fields(
        len = pdu.get().len(),
    )
)]
pub async fn parse_incoming_pdu(&self, pdu: &RawJsonValue) -> Result<Parsed> {
	let value: CanonicalJsonObject = serde_json::from_str(pdu.get()).map_err(|e| {
		err!(BadServerResponse(debug_error!("Error parsing incoming event: {e} {pdu:#?}")))
	})?;

	let room_id: OwnedRoomId = value
		.get("room_id")
		.and_then(CanonicalJsonValue::as_str)
		.map(OwnedRoomId::parse)
		.flat_ok_or(err!(Request(InvalidParam("Invalid room_id in pdu"))))?;

	let room_version_id = self
		.services
		.state
		.get_room_version(&room_id)
		.await
		.map_err(|_| err!("Server is not in room {room_id}"))?;

	gen_event_id(&value, &room_version_id)
		.map(move |event_id| (room_id, event_id, value))
		.map_err(|e| {
			err!(Request(InvalidParam("Could not convert event to canonical json: {e}")))
		})
}
