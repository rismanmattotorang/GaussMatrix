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

use std::collections::BTreeSet;

use gm_api::StateEvent;
use gm_stateres::{
	AllOf, AuthRules, CreateRules, Event, EventId, EventStore, MembershipRules,
	PowerLevelMutationRules, PowerLevelRules, ResolvedStateCache, StateMap, resolve,
};
use ruma::OwnedEventId;

use crate::rooms::timeline;

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
