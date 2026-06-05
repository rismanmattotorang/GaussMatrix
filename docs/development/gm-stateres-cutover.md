# gm-stateres cutover seam

Scope for promoting the clean-room `gm-stateres` engine from its current
**shadow** role to the **authoritative** state-resolution path, mirroring the
gm-fed adoption pattern.

## Where we are

`gm-stateres` already resolves live room state, but only observationally:

- **`event_handler::resolve_state::state_resolution`**
  (`src/service/rooms/event_handler/resolve_state.rs:109`) collects the forks and
  auth-chains up front (lines 123–124) so both engines consume identical inputs,
  runs the legacy `state_res::resolve` to produce the **authoritative** result
  (lines 126–135), and — when `config.gm_stateres_shadow` is set — re-runs
  `gm_resolve::shadow_compare` over the same inputs and only *logs* agreement or
  divergence (lines 137–147).
- **`gm_resolve::shadow_compare` / `resolve_forks`** (`src/service/gm_resolve.rs`)
  bridge stored PDUs into `gm_api::StateEvent`s, build the resolution
  `EventStore`, and run the two-pass `resolve`.
- The flag `gm_stateres_shadow` (`src/core/config/mod.rs:821`, default off) gates
  the shadow; the legacy engine is always authoritative.

## What "cutover" means

Promote the `gm-stateres` result to the value `state_resolution` returns
(`Ok(resolved)`, `resolve_state.rs:149`), behind a new config flag, instead of
only shadow-comparing it.

## Blockers and work items

### 1. Sender power-level derivation — *prerequisite*

`Event::power_level()` is "the effective power level of this event's sender"
(`src/stateres/event.rs:24`) and is consumed by `reverse_topological_power_sort`
(`src/stateres/order.rs:113`) to order control events. `resolve_forks` built
every event with `StateEvent::from_view(&pdu, 0)` — a hard-coded **0**. In shadow
mode a wrong weight only mis-logs a divergence; authoritative, it mis-orders the
power sort and can resolve to the wrong event.

The authoritative path derives this in `power_level_for_sender`
(`src/service/rooms/state_res/resolve/power_sort.rs:190`): find the
`m.room.power_levels` event in the event's own auth chain and look up the
sender's level (`users[sender]`, else `users_default`), falling back to the
`users_default` default (0) when no power-levels event is in scope.

**Status: landed (this change).** `gm_resolve::resolve_forks` now derives each
event's sender level from the power-levels event in its auth chain via
`sender_power_level`, matching the common-case (room v1–v10) authoritative
behaviour.

**Follow-up:** room versions that privilege room creators (v11+/hydra) grant
creators an elevated level even absent a power-levels event. `StateEvent` does
not yet carry the create-event creator set or the room version, so the
creator-privilege case is not yet modelled here.

### 2. Output back-projection

`gm-stateres` returns the stringly-typed `StateMap` (`(String,String) → String`);
the production path returns `StateMap<OwnedEventId>` keyed by `TypeStateKey`.
`to_gm_statemap` (`gm_resolve.rs`) is the forward projection — cutover needs its
inverse (parse type/state-key back into `TypeStateKey`, event-id string back into
`OwnedEventId`), with explicit handling for unparseable keys rather than a silent
drop, plus a round-trip test against `to_gm_statemap`.

### 3. Missing-event handling

`resolve_forks` skips events absent from the timeline (`if let Ok(pdu) = …`).
Acceptable for an observational shadow; authoritative, a skipped auth event
silently changes the result. Cutover needs a fallible entry point
(`Result<LiveStateMap>`) that treats a missing referenced event as an error (or a
defined fallback to the legacy result), not a skip.

## Recommended shape — mirror gm-fed

gm-fed cut over behind `gm_fed_authoritative_sender` (`config/mod.rs:806`,
default off; gated at `service/fed/mod.rs:84,184`). Mirror it:

- Add `gm_stateres_authoritative: bool` (default off) alongside
  `gm_stateres_shadow`.
- Three-state rollout: **off** (legacy only) → **shadow** (observe, current) →
  **authoritative** (return the `gm-stateres` result; keep computing the legacy
  result as a divergence-logged safety net until confidence is high).
- A `gm_resolve` entry point returning `Result<LiveStateMap>` so
  `state_resolution` selects the source by flag.

## Suggested sequencing

1. **Sender PL derivation** — *done*.
2. `from_gm_statemap` inverse projection + round-trip test.
3. Fallible event store (missing event → `Err`).
4. `gm_stateres_authoritative` flag + the flag-driven branch in
   `state_resolution`.
5. Run the `state_res` snapshot suite (`src/service/tests/state_res/`) with the
   flag on to confirm parity with the legacy engine.

The shadow's own divergence logs (run with `gm_stateres_shadow = true` against
real traffic) are the gate: cutover should follow a clean shadow window.
