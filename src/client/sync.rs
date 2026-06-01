//! The simplified sliding-sync window (MSC4186, §V-C).

use std::collections::BTreeSet;

/// A client's view over a server-ordered room list, materialising only the
/// rooms in the requested index ranges.
///
/// The window is the heart of the lazy-loading client: only rooms inside the
/// ranges are materialised, so cold start cost scales with the window, not the
/// account. Expanding the ranges (the user scrolling) reveals the
/// newly-visible rooms to fetch and render.
#[derive(Clone, Debug, Default)]
pub struct SlidingWindow {
	/// The server-maintained room ordering (e.g. by recent activity).
	rooms: Vec<String>,
	/// The inclusive index ranges currently materialised.
	ranges: Vec<(usize, usize)>,
}

impl SlidingWindow {
	/// A window over `rooms` with nothing yet materialised.
	#[must_use]
	pub fn new(rooms: Vec<String>) -> Self { Self { rooms, ranges: Vec::new() } }

	/// The total number of rooms in the ordering.
	#[must_use]
	pub fn len(&self) -> usize { self.rooms.len() }

	/// Whether the ordering is empty.
	#[must_use]
	pub fn is_empty(&self) -> bool { self.rooms.is_empty() }

	/// Set the requested ranges, returning the rooms that become newly visible
	/// (in the new window but not the previous one) — the rooms the client must
	/// now fetch and render.
	pub fn request(&mut self, ranges: Vec<(usize, usize)>) -> Vec<String> {
		let previous = self.visible_indices();
		self.ranges = ranges;
		let current = self.visible_indices();

		current
			.difference(&previous)
			.filter_map(|&index| self.rooms.get(index).cloned())
			.collect()
	}

	/// The rooms currently materialised, in order.
	#[must_use]
	pub fn visible(&self) -> Vec<&str> {
		self.visible_indices()
			.into_iter()
			.filter_map(|index| self.rooms.get(index).map(String::as_str))
			.collect()
	}

	/// Replace the room ordering (a server reorder), keeping the same ranges.
	/// Returns the rooms now visible that were not visible before.
	pub fn reorder(&mut self, rooms: Vec<String>) -> Vec<String> {
		let previous: BTreeSet<String> = self.visible().into_iter().map(str::to_owned).collect();
		self.rooms = rooms;

		self.visible()
			.into_iter()
			.filter(|room| !previous.contains(*room))
			.map(str::to_owned)
			.collect()
	}

	/// The set of in-window indices, clamped to the room count and deduplicated
	/// across overlapping ranges.
	fn visible_indices(&self) -> BTreeSet<usize> {
		let count = self.rooms.len();
		let mut indices = BTreeSet::new();
		for &(start, end) in &self.ranges {
			if start >= count {
				continue;
			}
			let end = end.min(count.saturating_sub(1));
			for index in start..=end {
				indices.insert(index);
			}
		}

		indices
	}
}
