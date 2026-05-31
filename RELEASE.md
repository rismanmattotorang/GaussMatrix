# GaussMatrix 1.7.0

May 21, 2026

**All servers raising their `cache_capacity_modifier` above default must consider decreasing it to deploy this release.** The default of `1.0` is now generally recommended, and up to `2.0` for systems with four or less cores. Taking no action may cost 25% to 50% more memory (#123).

Nine additional MSCs have landed. Current status is kept up to date in our [documentation](https://gaussmatrix.dev/development/compliance/msc.html).

### New Features & Enhancements

- **Threaded read receipts and notifications** (MSC3771, MSC3773) ship across storage, federation, sync v3, SSS v5, and the push gateway. Existing users may see a one-time jump in unread or badge counts that previously sat at the main-context-only total.

- **MSC4225 one-time-key upload-order issuance** is implemented. **Operator note:** the legacy `onetimekeyid_onetimekeys` column is dropped on first read-write open and existing OTKs are erased; clients re-upload on their next /sync, with MSC2732 fallback keys (where present) covering the gap. The wipe is one-way; read-only and secondary replicas tolerate the missing column until the primary recreates it.

- **MSC4222 `state_after` on /sync v3** as an opt-in via `?use_state_after=true`. Clients that don't opt in are unaffected.

- **MSC4115 `unsigned.membership`** on served events from encrypted rooms. Informational; clients that ignore the field are unaffected.

- **Synapse-compatible shared-secret register endpoint** at `/_synapse/admin/v1/register`, raised by @iwalkalone69 in (#38). The admin module was also split into a directory of units.

- **Refresh-token expiry with optional hard logout** via three new keys: `refresh_token_ttl`, `refresh_token_idle_only`, and `refresh_token_hard_logout`. All default disabled.

- **Configurable extra OIDC authorization parameters**, courtesy of @Batmaev in (#450). Closes the silent-relogin gap with Logto and Element X when operators set `prompt = "login"`. Thank you!

- **MSC4452 `preview_url` capability** is advertised on /capabilities.

- **MSC4466 `propagate_to` profile query parameter** is implemented; the room fan-out also runs concurrently.

- **MSC3283 `m.set_displayname` and `m.set_avatar_url` capabilities** are advertised.

- **MSC3814 fallback keys on dehydrated-device PUT** are now stored.

- **OpenTelemetry OTLP exporter** replaces the deprecated `opentelemetry-jaeger` crate, closing (#85); `tokio-console` is re-enabled.

- Tip of the hat to @nyakokitsu, who reported in (#460) that `turn_uris` set without TURN credentials produces empty creds silently. GaussMatrix now warns at startup.

- Thanks to @dasha-uwu for simplifying `servers_route_via`.

- Per-cache defaults were rebalanced from observed utilization in (#123): `auth_chain` up 15x to 1.5M, several shorteventid/eventid caches 4x to 400k. Closes (#123) and (#423) opened by @scanash00; an earlier report by @alythemonk in (#262) on Oracle Linux OOM-via-PGTABLE is also addressed. Operators on `<= 2 GiB` hosts may want to clamp the cache modifiers in their toml to restore the previous baseline.

- Admin `db memory-usage` is now tabular `(used / cap / util%)` with per-pool block-cache rows.

- Configuration items are annotated in the generated `gaussmatrix-example.toml` to indicate runtime-reloadable vs restart-required.

- The KV codec's additive-tail invariant for trailing `Option<_>`, `&str`, and `&[u8]` fields is documented.

- A migration upgrades legacy `mediaid_user` keys to the composite layout.

- Thank you @NiklausHofer for the Gentoo Guru ebuild in "Getting Started" (#461).

- Docs: `enable_loopback_candidate` troubleshooting motivated by @Lama-Thematique in (#451), `ip_source` for reverse-proxy users, and an MSC table refresh (MSC3419 verified, MSC1957 n/a, rows for MSC4461 through MSC4474). Closes (#94) opened by @MrRinkana.

- Thanks to @winyadepla for the traefik MatrixRTC config in (#462) bringing parity with nginx and caddy. Also addresses (#69) opened by @GZEI.

- CI gained a Complement job-summary script, a Playwright stage, and drift detection so a missed `gaussmatrix-example.toml` regeneration fails check and clippy.

### Bug Fixes

- A v1.6.2 regression on non-S3 storage backends panicked the main thread on first upload. Reported by @Sommerwiesel in (#452); multipart is now gated on S3 only. Sincere apologies for the disruption.

- @BVollmerhaus graciously reported in (#454) that MSC2246 asynchronous media uploads could self-deadlock on the notifier mutex; the acquisition order is fixed. Thank you!

- Thanks to @digikar99, who reported in (#459) that the UIAA fallback acknowledgement rejected non-SSO flows; the registration token + password flow completes cleanly.

- Sliding-sync cached list ranges refresh on every explicit list update, shipped by @lhjt in (#455); previously a scrolled-to range was ignored indefinitely. Thank you!

- /threads and /backfill apply the visibility filter before pagination `take`, so a final non-empty page still returns `next_batch`.

- GitHub OIDC default `base_path` now aligns with their published discovery doc after they quietly changed the issuer (eb51c70ca, 6552f8668).

- OAuth Dynamic Client Registration records are bounded to a fixed size. Thanks @CEbbinghaus! (e5f625d89).

- OAuth SSO grant params win over operator-configured extras on key collision (05dba7ee9).

- Receipts and presence EDU emission is bounded below the federation budget (b4fcf5871).

- Membership tolerates stale room state on self-leave (39c72c233).

- A missed optimization in `/state` and map-value storage was corrected (b305e6a86); `/state` now also propagates per-PDU read errors instead of silently skipping corrupt events.

- Five route doc-headers had incorrect HTTP verbs (a40ca8f0a).

- The Docker bake file dropped the non-functional `cache_to`/`cache_from` directives (8e4bc8c68).

- Thank you @pedrompcaetano for the typo fix in `gaussmatrix.container` (#456).

- Stale comments and fan-out destructure cleanups (85e85c883, 9c4cd7c33, 2cc249363).
