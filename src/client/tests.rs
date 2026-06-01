//! Tests for the simplified sliding-sync window.

use crate::SlidingWindow;

fn rooms(count: usize) -> Vec<String> {
	(0..count).map(|i| format!("!room{i}")).collect()
}

#[test]
fn nothing_is_visible_until_a_range_is_requested() {
	let window = SlidingWindow::new(rooms(100));
	assert_eq!(window.len(), 100);
	assert!(window.visible().is_empty());
}

#[test]
fn requesting_a_window_materialises_only_that_slice() {
	let mut window = SlidingWindow::new(rooms(100));
	let newly = window.request(vec![(0, 9)]);
	assert_eq!(newly.len(), 10);
	assert_eq!(window.visible(), (0..10).map(|i| format!("!room{i}")).collect::<Vec<_>>());
}

#[test]
fn expanding_the_window_reveals_only_the_new_rooms() {
	let mut window = SlidingWindow::new(rooms(100));
	window.request(vec![(0, 9)]);
	// Scroll: expand the window to the first 20 rooms.
	let newly = window.request(vec![(0, 19)]);
	assert_eq!(newly, (10..20).map(|i| format!("!room{i}")).collect::<Vec<_>>());
	assert_eq!(window.visible().len(), 20);
}

#[test]
fn ranges_are_clamped_and_overlaps_deduplicated() {
	let mut window = SlidingWindow::new(rooms(5));
	// End past the list and an overlapping range.
	window.request(vec![(0, 2), (1, 99)]);
	assert_eq!(window.visible().len(), 5, "clamped to the 5 rooms, no duplicates");
}

#[test]
fn reorder_reveals_rooms_moved_into_the_window() {
	let mut window = SlidingWindow::new(rooms(100));
	window.request(vec![(0, 4)]); // rooms 0..5 visible
	// A previously out-of-window room becomes most-recent (moves to index 0).
	let mut reordered = rooms(100);
	reordered.insert(0, "!busy".to_owned());
	let newly = window.reorder(reordered);
	assert_eq!(newly, vec!["!busy".to_owned()], "the room that scrolled into view");
}
