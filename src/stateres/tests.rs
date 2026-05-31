//! Unit tests for partitioning, the resolved-state cache, and resolution.

use std::{
	cell::Cell,
	collections::{BTreeMap, BTreeSet},
};

use crate::{
	AllOf, AuthRules, ConflictedState, CreateRules, Event, EventId, EventStore, MembershipRules,
	PowerLevelRules, PowerLevels, ResolvedStateCache, StateKey, StateMap, auth_difference,
	conflicting_event_ids, iterative_auth_checks, mainline_ordering, partition, resolve,
	reverse_topological_power_sort,
};

/// A test event carrying just the fields the ordering and auth rules need.
struct TestEvent {
	id: EventId,
	kind: String,
	state_key: String,
	sender: String,
	power: i64,
	ts: u64,
	auth: Vec<EventId>,
	power_levels: Option<PowerLevels>,
	membership: Option<String>,
	join_rule: Option<String>,
}

impl TestEvent {
	fn new(id: &str, power: i64, ts: u64, auth: &[&str]) -> Self {
		Self::typed(id, "", power, ts, auth)
	}

	fn typed(id: &str, kind: &str, power: i64, ts: u64, auth: &[&str]) -> Self {
		Self {
			id: id.to_owned(),
			kind: kind.to_owned(),
			state_key: String::new(),
			sender: String::new(),
			power,
			ts,
			auth: auth.iter().map(|a| (*a).to_owned()).collect(),
			power_levels: None,
			membership: None,
			join_rule: None,
		}
	}

	/// An `m.room.member` event setting `target`'s membership, sent by `sender`.
	fn member(id: &str, sender: &str, target: &str, membership: &str, auth: &[&str]) -> Self {
		let mut event = Self::typed(id, "m.room.member", 0, 0, auth);
		event.sender = sender.to_owned();
		event.state_key = target.to_owned();
		event.membership = Some(membership.to_owned());
		event
	}

	/// An `m.room.join_rules` event setting the room's join rule.
	fn joinrules(id: &str, rule: &str, auth: &[&str]) -> Self {
		let mut event = Self::typed(id, "m.room.join_rules", 0, 0, auth);
		event.join_rule = Some(rule.to_owned());
		event
	}

	fn state_event(id: &str, kind: &str, state_key: &str, auth: &[&str]) -> Self {
		let mut event = Self::typed(id, kind, 0, 0, auth);
		event.state_key = state_key.to_owned();
		event
	}

	/// An event of `kind` sent by `sender`.
	fn by(id: &str, kind: &str, sender: &str, auth: &[&str]) -> Self {
		let mut event = Self::typed(id, kind, 0, 0, auth);
		event.sender = sender.to_owned();
		event
	}

	/// An `m.room.power_levels` event carrying `levels`.
	fn powerlevels(id: &str, levels: PowerLevels, auth: &[&str]) -> Self {
		let mut event = Self::typed(id, "m.room.power_levels", 0, 0, auth);
		event.power_levels = Some(levels);
		event
	}
}

impl Event for TestEvent {
	fn event_id(&self) -> &str { &self.id }

	fn event_type(&self) -> &str { &self.kind }

	fn state_key(&self) -> &str { &self.state_key }

	fn power_level(&self) -> i64 { self.power }

	fn origin_server_ts(&self) -> u64 { self.ts }

	fn auth_event_ids(&self) -> &[EventId] { &self.auth }

	fn sender(&self) -> &str { &self.sender }

	fn power_levels(&self) -> Option<&PowerLevels> { self.power_levels.as_ref() }

	fn membership(&self) -> Option<&str> { self.membership.as_deref() }

	fn join_rule(&self) -> Option<&str> { self.join_rule.as_deref() }
}

/// Build a by-id store from a set of events.
fn store(events: Vec<TestEvent>) -> BTreeMap<EventId, TestEvent> {
	events.into_iter().map(|e| (e.id.clone(), e)).collect()
}

fn id_set(ids: &[&str]) -> BTreeSet<EventId> {
	ids.iter().map(|s| (*s).to_owned()).collect()
}

fn key(event_type: &str, state_key: &str) -> StateKey {
	(event_type.to_owned(), state_key.to_owned())
}

/// Build a state map from `(type, state_key, event_id)` triples.
fn state(pairs: &[(&str, &str, &str)]) -> StateMap {
	pairs
		.iter()
		.map(|(t, s, e)| (key(t, s), (*e).to_owned()))
		.collect()
}

/// A deterministic stand-in for the room-version auth ordering: pick the
/// lexicographically smallest event id per conflicted slot.
fn order_smallest(conflicted: &ConflictedState) -> StateMap {
	conflicted
		.iter()
		.map(|(k, ids)| (k.clone(), ids.iter().next().cloned().unwrap()))
		.collect()
}

#[test]
fn partition_all_agree_is_unconflicted() {
	let a = state(&[("m.room.name", "", "$n1"), ("m.room.topic", "", "$t1")]);
	let b = a.clone();
	let p = partition(&[a, b]);
	assert_eq!(p.unconflicted.len(), 2);
	assert!(p.conflicted.is_empty());
	assert_eq!(p.unconflicted.get(&key("m.room.name", "")).map(String::as_str), Some("$n1"));
}

#[test]
fn partition_disagreement_is_conflicted() {
	let a = state(&[("m.room.name", "", "$n1")]);
	let b = state(&[("m.room.name", "", "$n2")]);
	let p = partition(&[a, b]);
	assert!(p.unconflicted.is_empty());
	assert_eq!(p.conflicted.len(), 1);
	assert_eq!(p.conflicted[&key("m.room.name", "")].len(), 2);
}

#[test]
fn partition_absent_in_some_is_still_unconflicted() {
	// A slot present in only one map, with a single value, is not a conflict.
	let a = state(&[("m.room.name", "", "$n1"), ("m.room.topic", "", "$t1")]);
	let b = state(&[("m.room.name", "", "$n1")]);
	let p = partition(&[a, b]);
	assert!(p.conflicted.is_empty());
	assert_eq!(p.unconflicted.len(), 2);
}

#[test]
fn conflicting_event_ids_flattens_all_disputed_events() {
	let a = state(&[("m.room.name", "", "$n1"), ("m.room.topic", "", "$t1")]);
	let b = state(&[("m.room.name", "", "$n2"), ("m.room.topic", "", "$t2")]);
	let p = partition(&[a, b]);
	let ids = conflicting_event_ids(&p.conflicted);
	assert_eq!(ids.len(), 4);
	assert!(ids.contains("$n1") && ids.contains("$t2"));
}

#[test]
fn cache_insert_and_get() {
	let mut cache = ResolvedStateCache::new(4);
	let k = conflicting_event_ids(&partition(&[state(&[("a", "", "$1")]), state(&[("a", "", "$2")])]).conflicted);
	assert!(cache.get(&k).is_none());
	cache.insert(k.clone(), state(&[("a", "", "$1")]));
	assert_eq!(cache.get(&k).unwrap().get(&key("a", "")).map(String::as_str), Some("$1"));
	assert_eq!(cache.len(), 1);
	assert!(!cache.is_empty());
}

#[test]
fn cache_evicts_least_recently_inserted_over_capacity() {
	let mut cache = ResolvedStateCache::new(2);
	let mk = |id: &str| -> BTreeSet<String> {
		let mut s = BTreeSet::new();
		s.insert(id.to_owned());
		s
	};
	let (a, b, c) = (mk("$a"), mk("$b"), mk("$c"));
	cache.insert(a.clone(), StateMap::new());
	cache.insert(b.clone(), StateMap::new());
	cache.insert(c.clone(), StateMap::new());
	assert_eq!(cache.len(), 2);
	assert!(cache.get(&a).is_none(), "oldest entry should have been evicted");
	assert!(cache.get(&b).is_some());
	assert!(cache.get(&c).is_some());
}

#[test]
fn cache_zero_capacity_disables_caching() {
	let mut cache = ResolvedStateCache::new(0);
	let mut k = BTreeSet::new();
	k.insert("$x".to_owned());
	cache.insert(k.clone(), StateMap::new());
	assert!(cache.get(&k).is_none());
	assert!(cache.is_empty());
}

#[test]
fn cache_reinsert_refreshes_value_without_duplicate_slot() {
	let mut cache = ResolvedStateCache::new(8);
	let mut k = BTreeSet::new();
	k.insert("$x".to_owned());
	cache.insert(k.clone(), state(&[("a", "", "$old")]));
	cache.insert(k.clone(), state(&[("a", "", "$new")]));
	assert_eq!(cache.len(), 1);
	assert_eq!(cache.get(&k).unwrap().get(&key("a", "")).map(String::as_str), Some("$new"));
}

#[test]
fn resolve_without_conflict_returns_unconflicted() {
	let a = state(&[("m.room.name", "", "$n1")]);
	let b = a.clone();
	let mut cache = ResolvedStateCache::new(4);
	let resolved = resolve(&[a, b], &mut cache, order_smallest);
	assert_eq!(resolved.len(), 1);
	assert!(cache.is_empty(), "no conflict means nothing to memoise");
}

#[test]
fn resolve_orders_conflict_and_merges_with_unconflicted() {
	let a = state(&[("m.room.name", "", "$n1"), ("m.room.topic", "", "$shared")]);
	let b = state(&[("m.room.name", "", "$n2"), ("m.room.topic", "", "$shared")]);
	let mut cache = ResolvedStateCache::new(4);
	let resolved = resolve(&[a, b], &mut cache, order_smallest);

	// topic was unconflicted; name was resolved to the smallest id ($n1 < $n2).
	assert_eq!(resolved.get(&key("m.room.topic", "")).map(String::as_str), Some("$shared"));
	assert_eq!(resolved.get(&key("m.room.name", "")).map(String::as_str), Some("$n1"));
	assert_eq!(cache.len(), 1);
}

#[test]
fn resolve_memoises_and_skips_reordering_on_cache_hit() {
	let a = state(&[("m.room.name", "", "$n1")]);
	let b = state(&[("m.room.name", "", "$n2")]);
	let calls = Cell::new(0_u32);
	let mut cache = ResolvedStateCache::new(4);

	let counting = |c: &ConflictedState| {
		calls.set(calls.get().saturating_add(1));
		order_smallest(c)
	};

	let first = resolve(&[a.clone(), b.clone()], &mut cache, counting);
	let second = resolve(&[a, b], &mut cache, counting);

	assert_eq!(first, second);
	assert_eq!(calls.get(), 1, "ordering must run once; the second resolve hits the cache");
}

#[test]
fn auth_difference_is_union_minus_intersection() {
	let forks = [id_set(&["$a", "$b", "$c"]), id_set(&["$b", "$c", "$d"])];
	// $b and $c are shared; $a and $d differ.
	assert_eq!(auth_difference(&forks), id_set(&["$a", "$d"]));
}

#[test]
fn auth_difference_empty_when_all_agree() {
	let forks = [id_set(&["$a", "$b"]), id_set(&["$a", "$b"])];
	assert!(auth_difference(&forks).is_empty());
	assert!(auth_difference(&[]).is_empty());
}

#[test]
fn power_sort_orders_auth_events_before_dependents() {
	// $c authed by $b, $b authed by $a → topological order must be $a, $b, $c.
	let events = [
		TestEvent::new("$c", 100, 1, &["$b"]),
		TestEvent::new("$a", 100, 1, &[]),
		TestEvent::new("$b", 100, 1, &["$a"]),
	];
	assert_eq!(reverse_topological_power_sort(&events), vec!["$a", "$b", "$c"]);
}

#[test]
fn power_sort_breaks_ties_by_power_then_ts_then_id() {
	// Three independent events (no auth deps among them).
	let events = [
		// lower power, should come last among these
		TestEvent::new("$low", 10, 5, &[]),
		// highest power, comes first
		TestEvent::new("$high", 100, 9, &[]),
		// mid power, two with equal power broken by ts then id
		TestEvent::new("$mid_late", 50, 20, &[]),
		TestEvent::new("$mid_early", 50, 10, &[]),
	];
	// highest power first; equal-power pair ordered by earlier ts; lowest last.
	assert_eq!(
		reverse_topological_power_sort(&events),
		vec!["$high", "$mid_early", "$mid_late", "$low"]
	);
}

#[test]
fn power_sort_ignores_auth_events_outside_the_set() {
	// $a's auth event $external is not in the set and must not block emission.
	let events = [TestEvent::new("$a", 50, 1, &["$external"])];
	assert_eq!(reverse_topological_power_sort(&events), vec!["$a"]);
}

#[test]
fn power_sort_equal_keys_broken_by_event_id() {
	let events = [
		TestEvent::new("$b", 50, 10, &[]),
		TestEvent::new("$a", 50, 10, &[]),
	];
	// Identical power and ts → smaller id first.
	assert_eq!(reverse_topological_power_sort(&events), vec!["$a", "$b"]);
}

fn ids(list: &[&str]) -> Vec<EventId> { list.iter().map(|s| (*s).to_owned()).collect() }

#[test]
fn mainline_orders_by_closest_power_levels_ancestor() {
	// Mainline of power-levels events: $pl0 (root) <- $pl1 <- $pl2 (resolved).
	let s = store(vec![
		TestEvent::typed("$pl0", "m.room.power_levels", 100, 1, &[]),
		TestEvent::typed("$pl1", "m.room.power_levels", 100, 2, &["$pl0"]),
		TestEvent::typed("$pl2", "m.room.power_levels", 100, 3, &["$pl1"]),
		TestEvent::new("$eC", 50, 30, &["$pl2"]),
		TestEvent::new("$eA", 50, 10, &["$pl0"]),
		TestEvent::new("$eB", 50, 20, &["$pl1"]),
	]);
	// Depth: $eA→$pl0 (0), $eB→$pl1 (1), $eC→$pl2 (2). Ascending by depth.
	let order = mainline_ordering(Some("$pl2"), &ids(&["$eC", "$eA", "$eB"]), &s);
	assert_eq!(order, vec!["$eA", "$eB", "$eC"]);
}

#[test]
fn mainline_breaks_ties_by_ts_then_id() {
	// All three anchor to the same power-levels event → equal depth.
	let s = store(vec![
		TestEvent::typed("$pl0", "m.room.power_levels", 100, 1, &[]),
		TestEvent::new("$late", 50, 99, &["$pl0"]),
		TestEvent::new("$early", 50, 5, &["$pl0"]),
		TestEvent::new("$b", 50, 5, &["$pl0"]),
	]);
	// ts orders $early/$b (5) before $late (99); equal ts → smaller id ($b < $early).
	let order = mainline_ordering(Some("$pl0"), &ids(&["$late", "$early", "$b"]), &s);
	assert_eq!(order, vec!["$b", "$early", "$late"]);
}

#[test]
fn mainline_without_power_levels_orders_by_ts_then_id() {
	let s = store(vec![
		TestEvent::new("$y", 0, 20, &[]),
		TestEvent::new("$x", 0, 10, &[]),
	]);
	// No mainline → all depth zero → ordered by ts.
	let order = mainline_ordering(None, &ids(&["$y", "$x"]), &s);
	assert_eq!(order, vec!["$x", "$y"]);
}

/// Authorise every event.
struct AcceptAll;
impl AuthRules<TestEvent> for AcceptAll {
	fn is_authorized(
		&self,
		_event: &TestEvent,
		_state: &StateMap,
		_store: &EventStore<TestEvent>,
	) -> bool {
		true
	}
}

#[test]
fn iterative_auth_checks_folds_authorized_events_into_state() {
	let s = store(vec![
		TestEvent::state_event("$pl", "m.room.power_levels", "", &[]),
		TestEvent::state_event("$name", "m.room.name", "", &["$pl"]),
	]);
	let resolved = iterative_auth_checks(&ids(&["$pl", "$name"]), StateMap::new(), &s, &AcceptAll);
	assert_eq!(resolved.get(&key("m.room.power_levels", "")).map(String::as_str), Some("$pl"));
	assert_eq!(resolved.get(&key("m.room.name", "")).map(String::as_str), Some("$name"));
}

#[test]
fn iterative_auth_checks_skips_rejected_events() {
	struct RejectBad;
	impl AuthRules<TestEvent> for RejectBad {
		fn is_authorized(
			&self,
			event: &TestEvent,
			_state: &StateMap,
			_store: &EventStore<TestEvent>,
		) -> bool {
			event.event_id() != "$bad"
		}
	}

	let s = store(vec![
		TestEvent::state_event("$good", "m.room.name", "", &[]),
		TestEvent::state_event("$bad", "m.room.topic", "", &[]),
	]);
	let resolved = iterative_auth_checks(&ids(&["$good", "$bad"]), StateMap::new(), &s, &RejectBad);
	assert!(resolved.contains_key(&key("m.room.name", "")));
	assert!(!resolved.contains_key(&key("m.room.topic", "")));
}

#[test]
fn iterative_auth_checks_sees_state_resolved_so_far() {
	// "$child" is only authorised once a power-levels event is in the state.
	struct NeedsPowerLevels;
	impl AuthRules<TestEvent> for NeedsPowerLevels {
		fn is_authorized(
			&self,
			event: &TestEvent,
			state: &StateMap,
			_store: &EventStore<TestEvent>,
		) -> bool {
			event.event_id() != "$child"
				|| state.contains_key(&key("m.room.power_levels", ""))
		}
	}

	let s = store(vec![
		TestEvent::state_event("$pl", "m.room.power_levels", "", &[]),
		TestEvent::state_event("$child", "m.room.name", "", &[]),
	]);

	// Power levels first → child is authorised.
	let ok = iterative_auth_checks(&ids(&["$pl", "$child"]), StateMap::new(), &s, &NeedsPowerLevels);
	assert!(ok.contains_key(&key("m.room.name", "")));

	// Child first → power levels not yet present → child is rejected.
	let bad = iterative_auth_checks(&ids(&["$child", "$pl"]), StateMap::new(), &s, &NeedsPowerLevels);
	assert!(!bad.contains_key(&key("m.room.name", "")));
}

#[test]
fn iterative_auth_checks_preserves_base_state() {
	let s = store(vec![TestEvent::state_event("$new", "m.room.name", "", &[])]);
	let mut base = StateMap::new();
	base.insert(key("m.room.topic", ""), "$existing".to_owned());
	let resolved = iterative_auth_checks(&ids(&["$new"]), base, &s, &AcceptAll);
	assert_eq!(resolved.get(&key("m.room.topic", "")).map(String::as_str), Some("$existing"));
	assert_eq!(resolved.get(&key("m.room.name", "")).map(String::as_str), Some("$new"));
}

fn levels() -> PowerLevels {
	PowerLevels {
		users: BTreeMap::from([("@admin".to_owned(), 100_i64)]),
		users_default: 10,
		events: BTreeMap::from([("m.room.name".to_owned(), 60_i64)]),
		events_default: 0,
		state_default: 50,
		invite: 0,
		kick: 50,
		ban: 50,
	}
}

/// Power levels giving `@admin` 100 and `@mod` 60, with default `users_default`
/// 0 and the standard invite/kick/ban gates.
fn membership_levels() -> PowerLevels {
	PowerLevels {
		users: BTreeMap::from([("@admin".to_owned(), 100_i64), ("@mod".to_owned(), 60_i64)]),
		users_default: 0,
		events: BTreeMap::new(),
		events_default: 0,
		state_default: 50,
		invite: 50,
		kick: 50,
		ban: 50,
	}
}

#[test]
fn power_levels_lookups_fall_back_to_defaults() {
	let pl = levels();
	assert_eq!(pl.for_user("@admin"), 100);
	assert_eq!(pl.for_user("@nobody"), 10);
	assert_eq!(pl.required_for("m.room.name", true), 60); // explicit
	assert_eq!(pl.required_for("m.room.topic", true), 50); // state_default
	assert_eq!(pl.required_for("m.room.message", false), 0); // events_default
}

#[test]
fn power_level_rules_authorize_by_sender_level() {
	let s = store(vec![
		TestEvent::powerlevels("$pl", levels(), &[]),
		// @admin (100) may send a topic (needs state_default 50).
		TestEvent::by("$ok", "m.room.topic", "@admin", &["$pl"]),
		// @bob (default 10) may not (needs 50).
		TestEvent::by("$no", "m.room.topic", "@bob", &["$pl"]),
	]);
	let mut state = StateMap::new();
	state.insert(key("m.room.power_levels", ""), "$pl".to_owned());

	let rules = PowerLevelRules;
	assert!(rules.is_authorized(&s["$ok"], &state, &s));
	assert!(!rules.is_authorized(&s["$no"], &state, &s));
}

#[test]
fn power_level_rules_drive_iterative_auth_checks() {
	let s = store(vec![
		TestEvent::powerlevels("$pl", levels(), &[]),
		TestEvent::by("$ok", "m.room.topic", "@admin", &["$pl"]),
		TestEvent::by("$no", "m.room.guest_access", "@bob", &["$pl"]),
	]);
	let mut base = StateMap::new();
	base.insert(key("m.room.power_levels", ""), "$pl".to_owned());

	let resolved = iterative_auth_checks(&ids(&["$ok", "$no"]), base, &s, &PowerLevelRules);
	assert!(resolved.contains_key(&key("m.room.topic", "")), "@admin authorised");
	assert!(!resolved.contains_key(&key("m.room.guest_access", "")), "@bob rejected");
}

#[test]
fn power_level_rules_default_to_permissive_without_power_levels() {
	// No power-levels event in state → default (all-zero) levels → authorised.
	let s = store(vec![TestEvent::by("$e", "m.room.name", "@anyone", &[])]);
	let rules = PowerLevelRules;
	assert!(rules.is_authorized(&s["$e"], &StateMap::new(), &s));
}

#[test]
fn create_rules_authorize_the_room_root() {
	let s = store(vec![TestEvent::state_event("$create", "m.room.create", "", &[])]);
	// A create event with no auth events and an empty state key is the root.
	assert!(CreateRules.is_authorized(&s["$create"], &StateMap::new(), &s));
}

#[test]
fn create_rules_reject_create_with_auth_events() {
	let s = store(vec![TestEvent::state_event("$bad", "m.room.create", "", &["$x"])]);
	assert!(!CreateRules.is_authorized(&s["$bad"], &StateMap::new(), &s));
}

#[test]
fn create_rules_require_the_room_to_exist_for_other_events() {
	let s = store(vec![TestEvent::state_event("$name", "m.room.name", "", &[])]);
	let rules = CreateRules;

	// No create event in state → rejected.
	assert!(!rules.is_authorized(&s["$name"], &StateMap::new(), &s));

	// Create event present → accepted.
	let mut state = StateMap::new();
	state.insert(key("m.room.create", ""), "$create".to_owned());
	assert!(rules.is_authorized(&s["$name"], &state, &s));
}

#[test]
fn all_of_requires_every_rule_to_pass() {
	let s = store(vec![
		TestEvent::powerlevels("$pl", levels(), &[]),
		TestEvent::by("$admin_evt", "m.room.topic", "@admin", &["$create", "$pl"]),
		TestEvent::by("$bob_evt", "m.room.topic", "@bob", &["$create", "$pl"]),
	]);
	let mut state = StateMap::new();
	state.insert(key("m.room.create", ""), "$create".to_owned());
	state.insert(key("m.room.power_levels", ""), "$pl".to_owned());

	let components: [&dyn AuthRules<TestEvent>; 2] = [&CreateRules, &PowerLevelRules];
	let rules = AllOf(&components);

	// @admin passes both gates (room exists, power 100 >= 50).
	assert!(rules.is_authorized(&s["$admin_evt"], &state, &s));
	// @bob passes create but fails power (10 < 50) → overall rejected.
	assert!(!rules.is_authorized(&s["$bob_evt"], &state, &s));

	// Even @admin is rejected when the create gate fails (no create in state).
	let mut without_create = StateMap::new();
	without_create.insert(key("m.room.power_levels", ""), "$pl".to_owned());
	assert!(!rules.is_authorized(&s["$admin_evt"], &without_create, &s));
}

#[test]
fn membership_rules_pass_non_member_events() {
	let s = store(vec![TestEvent::state_event("$name", "m.room.name", "", &[])]);
	assert!(MembershipRules.is_authorized(&s["$name"], &StateMap::new(), &s));
}

#[test]
fn membership_join_public_allowed_invite_requires_invitation() {
	let s = store(vec![
		TestEvent::joinrules("$public", "public", &[]),
		TestEvent::joinrules("$invite", "invite", &[]),
		TestEvent::member("$join", "@alice", "@alice", "join", &[]),
		TestEvent::member("$inv", "@bob", "@alice", "invite", &[]),
	]);

	// Public room: anyone may join.
	let mut public = StateMap::new();
	public.insert(key("m.room.join_rules", ""), "$public".to_owned());
	assert!(MembershipRules.is_authorized(&s["$join"], &public, &s));

	// Invite-only, alice not invited → rejected.
	let mut invite_only = StateMap::new();
	invite_only.insert(key("m.room.join_rules", ""), "$invite".to_owned());
	assert!(!MembershipRules.is_authorized(&s["$join"], &invite_only, &s));

	// Invite-only, alice invited → allowed.
	let mut invited = invite_only.clone();
	invited.insert(key("m.room.member", "@alice"), "$inv".to_owned());
	assert!(MembershipRules.is_authorized(&s["$join"], &invited, &s));
}

#[test]
fn membership_invite_requires_joined_sender_with_power() {
	let s = store(vec![
		TestEvent::powerlevels("$pl", membership_levels(), &[]),
		TestEvent::member("$adminjoin", "@admin", "@admin", "join", &[]),
		TestEvent::member("$bobjoin", "@bob", "@bob", "join", &[]),
		TestEvent::member("$inv_ok", "@admin", "@carol", "invite", &[]),
		TestEvent::member("$inv_low", "@bob", "@dave", "invite", &[]),
	]);
	let mut state = StateMap::new();
	state.insert(key("m.room.power_levels", ""), "$pl".to_owned());
	state.insert(key("m.room.member", "@admin"), "$adminjoin".to_owned());
	state.insert(key("m.room.member", "@bob"), "$bobjoin".to_owned());

	// @admin (joined, power 100 >= invite 50) invites carol.
	assert!(MembershipRules.is_authorized(&s["$inv_ok"], &state, &s));
	// @bob (joined, power 0 < invite 50) cannot invite.
	assert!(!MembershipRules.is_authorized(&s["$inv_low"], &state, &s));
}

#[test]
fn membership_leave_self_and_kick_and_ban_by_power() {
	let s = store(vec![
		TestEvent::powerlevels("$pl", membership_levels(), &[]),
		TestEvent::member("$alicejoin", "@alice", "@alice", "join", &[]),
		TestEvent::member("$modjoin", "@mod", "@mod", "join", &[]),
		TestEvent::member("$aliceleave", "@alice", "@alice", "leave", &[]),
		TestEvent::member("$kick", "@mod", "@alice", "leave", &[]),
		TestEvent::member("$kick_fail", "@alice", "@mod", "leave", &[]),
		TestEvent::member("$ban", "@mod", "@alice", "ban", &[]),
	]);
	let mut state = StateMap::new();
	state.insert(key("m.room.power_levels", ""), "$pl".to_owned());
	state.insert(key("m.room.member", "@alice"), "$alicejoin".to_owned());
	state.insert(key("m.room.member", "@mod"), "$modjoin".to_owned());

	// Alice (joined) leaves herself.
	assert!(MembershipRules.is_authorized(&s["$aliceleave"], &state, &s));
	// Mod (power 60 >= kick 50, 60 > 0) kicks alice.
	assert!(MembershipRules.is_authorized(&s["$kick"], &state, &s));
	// Alice (power 0) cannot kick the mod.
	assert!(!MembershipRules.is_authorized(&s["$kick_fail"], &state, &s));
	// Mod (power 60 >= ban 50, 60 > 0) bans alice.
	assert!(MembershipRules.is_authorized(&s["$ban"], &state, &s));
}
