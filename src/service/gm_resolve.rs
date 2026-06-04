//! State resolution over live PDUs via the gm-stateres engine (SPECS §III-D).
//!
//! Bridges the running server's stored events into the clean-room
//! state-resolution engine: each referenced event is fetched from the timeline,
//! projected into the resolution model through gm-api's `EventView`
//! ([`StateEvent::from_view`]), and the forks are resolved with the
//! room-version authorisation rules.
//!
//! This is the adoption seam for the engine; the production cutover from the
//! existing `rooms::state_res` path and the sender power-level derivation it
//! needs are follow-ups (sender levels are supplied as zero here).

use std::collections::{BTreeMap, BTreeSet};

use gaussmatrix_core::{debug, matrix::TypeStateKey, warn};
use gm_api::StateEvent;
use gm_stateres::{
	AllOf, AuthRules, CreateRules, Event, EventId, EventStore, MembershipRules,
	PowerLevelMutationRules, PowerLevelRules, ResolvedStateCache, StateMap, resolve,
};
use ruma::OwnedEventId;

use crate::rooms::timeline;

/// The running server's state map, keyed by event type + state key, as produced
/// by the production `rooms::state_res` path.
pub type LiveStateMap = BTreeMap<TypeStateKey, OwnedEventId>;

/// Resolve a set of state `forks` over live PDUs.
///
/// `event_ids` are the events referenced by the forks and their auth chains;
/// each is fetched from `timeline`, bridged into a [`StateEvent`], and used to
/// build the resolution event store. The two-pass resolve then runs with the
/// composed room-version auth rules. Events missing from the timeline are
/// skipped.
pub async fn resolve_forks(
	timeline: &timeline::Service,
	forks: &[StateMap],
	auth_chains: &[BTreeSet<EventId>],
	event_ids: &[OwnedEventId],
) -> StateMap {
	let mut store = EventStore::<StateEvent>::new();
	for id in event_ids {
		if let Ok(pdu) = timeline.get_pdu(id).await {
			let event = StateEvent::from_view(&pdu, 0);
			store.insert(event.event_id().to_owned(), event);
		}
	}

	let components: [&dyn AuthRules<StateEvent>; 4] =
		[&CreateRules, &PowerLevelRules, &PowerLevelMutationRules, &MembershipRules];
	let rules = AllOf(&components);
	let mut cache = ResolvedStateCache::new(256);

	resolve(forks, auth_chains, &store, &rules, &mut cache)
}

/// Project a live state map into the gm-stateres [`StateMap`] representation
/// (stringly-typed type/state-key tuples and event ids).
fn to_gm_statemap(state: &LiveStateMap) -> StateMap {
	state
		.iter()
		.map(|((ty, sk), id)| ((ty.to_string(), sk.to_string()), id.as_str().to_owned()))
		.collect()
}

/// Run the gm-stateres engine in shadow mode over a live resolution and log
/// whether it agrees with the `authoritative` result from the production path.
///
/// This is observation only: the caller has already computed and is about to
/// return `authoritative`; nothing here feeds back into the live path. The
/// shadow resolves the same `forks` and `auth_chains` (the very inputs the
/// production resolver consumed) and compares the two full resolved state maps
/// key by key, emitting a `debug` line on full agreement and a `warn` with the
/// divergence count (and a bounded sample) otherwise.
pub async fn shadow_compare(
	timeline: &timeline::Service,
	forks: &[LiveStateMap],
	auth_chains: &[BTreeSet<OwnedEventId>],
	authoritative: &LiveStateMap,
) {
	// Bridge the live-typed inputs into the engine's representation.
	let gm_forks: Vec<StateMap> = forks.iter().map(to_gm_statemap).collect();
	let gm_auth_chains: Vec<BTreeSet<EventId>> = auth_chains
		.iter()
		.map(|chain| {
			chain
				.iter()
				.map(|id| id.as_str().to_owned())
				.collect()
		})
		.collect();

	// Every event referenced by a fork or an auth chain may be needed to resolve.
	let mut referenced: BTreeSet<OwnedEventId> = BTreeSet::new();
	for fork in forks {
		referenced.extend(fork.values().cloned());
	}
	for chain in auth_chains {
		referenced.extend(chain.iter().cloned());
	}
	let event_ids: Vec<OwnedEventId> = referenced.into_iter().collect();

	let shadow = resolve_forks(timeline, &gm_forks, &gm_auth_chains, &event_ids).await;
	let expected = to_gm_statemap(authoritative);

	let divergences = diverging_keys(&expected, &shadow);
	if divergences.is_empty() {
		debug!(
			keys = expected.len(),
			"gm-stateres shadow agrees with the authoritative state resolution"
		);
	} else {
		// Bound the per-key sample so a large divergence can't flood the log.
		let sample: Vec<&(String, String)> = divergences.iter().take(8).copied().collect();
		warn!(
			authoritative_keys = expected.len(),
			shadow_keys = shadow.len(),
			divergences = divergences.len(),
			?sample,
			"gm-stateres shadow diverged from the authoritative state resolution"
		);
	}
}

/// The sorted, de-duplicated set of state keys on which `expected` and `shadow`
/// disagree, taken over the union of their keys. A key present in one map but
/// absent from the other counts as a divergence.
fn diverging_keys<'a>(expected: &'a StateMap, shadow: &'a StateMap) -> Vec<&'a (String, String)> {
	let mut divergences: Vec<&(String, String)> = expected
		.keys()
		.chain(shadow.keys())
		.filter(|key| expected.get(*key) != shadow.get(*key))
		.collect();
	divergences.sort_unstable();
	divergences.dedup();
	divergences
}

#[cfg(test)]
mod tests {
	use std::collections::BTreeMap;

	use ruma::{OwnedEventId, owned_event_id};

	use super::{LiveStateMap, StateMap, diverging_keys, to_gm_statemap};

	fn key(ty: &str, sk: &str) -> (String, String) { (ty.to_owned(), sk.to_owned()) }

	fn gm_map(entries: &[(&str, &str, &str)]) -> StateMap {
		entries
			.iter()
			.map(|(ty, sk, id)| (key(ty, sk), (*id).to_owned()))
			.collect()
	}

	#[test]
	fn live_map_projects_to_gm_statemap() {
		use ruma::events::StateEventType;

		let id: OwnedEventId = owned_event_id!("$abc:example.org");
		let mut live: LiveStateMap = BTreeMap::new();
		live.insert((StateEventType::RoomCreate, String::new().into()), id.clone());

		let gm = to_gm_statemap(&live);
		assert_eq!(gm.get(&key("m.room.create", "")), Some(&id.as_str().to_owned()));
		assert_eq!(gm.len(), 1);
	}

	#[test]
	fn identical_maps_have_no_divergence() {
		let a = gm_map(&[("m.room.create", "", "$c:x"), ("m.room.member", "@a:x", "$m:x")]);
		let b = a.clone();
		assert!(diverging_keys(&a, &b).is_empty());
	}

	#[test]
	fn differing_value_and_missing_key_both_diverge() {
		let expected =
			gm_map(&[("m.room.create", "", "$c:x"), ("m.room.member", "@a:x", "$m1:x")]);
		// Differs in the member value and adds a key the expected map lacks.
		let shadow = gm_map(&[
			("m.room.create", "", "$c:x"),
			("m.room.member", "@a:x", "$m2:x"),
			("m.room.topic", "", "$t:x"),
		]);

		let diverged = diverging_keys(&expected, &shadow);
		assert_eq!(diverged, vec![&key("m.room.member", "@a:x"), &key("m.room.topic", "")]);
	}
}
