//! The resolved-state cache.

use std::collections::{BTreeSet, HashMap, VecDeque};

use crate::state_map::{EventId, StateMap};

/// The key under which a resolution result is memoised: the set of conflicting
/// state-event identifiers that produced it.
type CacheKey = BTreeSet<EventId>;

/// Memoises state-resolution outputs keyed by the set of conflicting
/// state-event identifiers, so recurrent conflicts are not recomputed
/// (specification §III-D, front two).
///
/// Eviction is least-recently-inserted once `capacity` is exceeded, keeping the
/// cache bounded. A `capacity` of zero disables caching.
pub struct ResolvedStateCache {
	entries: HashMap<CacheKey, StateMap>,
	order: VecDeque<CacheKey>,
	capacity: usize,
}

impl ResolvedStateCache {
	/// Create a cache holding at most `capacity` resolution results.
	#[must_use]
	pub fn new(capacity: usize) -> Self {
		Self {
			entries: HashMap::new(),
			order: VecDeque::new(),
			capacity,
		}
	}

	/// The memoised resolution for `key`, if present.
	#[must_use]
	pub fn get(&self, key: &CacheKey) -> Option<&StateMap> { self.entries.get(key) }

	/// Memoise `resolved` under `key`, evicting the oldest entry if the cache is
	/// over capacity. A re-inserted key refreshes its value without duplicating
	/// its eviction-order slot.
	pub fn insert(&mut self, key: CacheKey, resolved: StateMap) {
		if self.capacity == 0 {
			return;
		}

		if self.entries.insert(key.clone(), resolved).is_none() {
			self.order.push_back(key);
		}

		while self.order.len() > self.capacity {
			if let Some(evicted) = self.order.pop_front() {
				self.entries.remove(&evicted);
			}
		}
	}

	/// The number of memoised results.
	#[must_use]
	pub fn len(&self) -> usize { self.entries.len() }

	/// Whether the cache holds no results.
	#[must_use]
	pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}
