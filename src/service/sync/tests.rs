use ruma::{
	UInt,
	api::client::sync::sync_events::v5::{ListId, Ranges, Request, request},
	events::StateEventType,
};

use super::Connection;

const LIST_ID: &str = "main";

#[test]
fn update_cache_replaces_existing_list_ranges() {
	let mut conn = Connection::default();

	conn.update_cache(&request_with_list(list_with_ranges(&[(0, 19)])));
	conn.update_cache(&request_with_list(list_with_ranges(&[(20, 39)])));

	assert_cached_ranges(&conn, &[(20, 39)]);
}

#[test]
fn update_cache_allows_empty_ranges_to_replace_existing_ranges() {
	let mut conn = Connection::default();

	conn.update_cache(&request_with_list(list_with_ranges(&[(0, 19)])));
	conn.update_cache(&request_with_list(list_with_ranges(&[])));

	assert_cached_ranges(&conn, &[]);
}

#[test]
fn update_cache_keeps_ranges_when_list_is_omitted() {
	let mut conn = Connection::default();

	conn.update_cache(&request_with_list(list_with_ranges(&[(0, 19)])));
	conn.update_cache(&Request::new());

	assert_cached_ranges(&conn, &[(0, 19)]);
}

#[test]
fn update_cache_preserves_sticky_list_metadata() {
	let mut conn = Connection::default();
	let required_state = vec![(StateEventType::RoomMember, "$LAZY".into())];

	conn.update_cache(&request_with_list(list_with_required_state(
		&[(0, 19)],
		required_state.clone(),
	)));
	conn.update_cache(&request_with_list(list_with_ranges(&[(20, 39)])));

	let cached = conn
		.lists
		.get(&list_id())
		.expect("list must remain cached");

	assert_eq!(cached.room_details.required_state, required_state);
	assert_cached_ranges(&conn, &[(20, 39)]);
}

fn request_with_list(list: request::List) -> Request {
	let mut request = Request::new();

	request.lists.insert(list_id(), list);

	request
}

fn list_with_ranges(ranges: &[(u64, u64)]) -> request::List {
	list_with_required_state(ranges, Vec::new())
}

fn list_with_required_state(
	ranges: &[(u64, u64)],
	required_state: Vec<(StateEventType, ruma::events::StateKey)>,
) -> request::List {
	request::List {
		ranges: ranges_from_u64(ranges),
		room_details: request::ListConfig { required_state, ..Default::default() },
		..Default::default()
	}
}

fn assert_cached_ranges(conn: &Connection, expected: &[(u64, u64)]) {
	let cached = conn
		.lists
		.get(&list_id())
		.expect("list must be cached");

	assert_eq!(cached.ranges, ranges_from_u64(expected));
}

fn ranges_from_u64(ranges: &[(u64, u64)]) -> Ranges {
	ranges
		.iter()
		.map(|&(start, end)| (uint(start), uint(end)))
		.collect()
}

fn uint(value: u64) -> UInt { UInt::new(value).expect("range value must fit UInt") }

fn list_id() -> ListId { LIST_ID.into() }
