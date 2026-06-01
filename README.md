# GaussMatrix

![License](https://img.shields.io/badge/license-Apache--2.0-8A2BE2?style=flat-square)
![Language](https://img.shields.io/badge/built%20with-Rust-8A2BE2?style=flat-square&logo=rust&logoColor=white)
![Protocol](https://img.shields.io/badge/protocol-Matrix-098A09?style=flat-square)
![Status](https://img.shields.io/badge/status-Phase%200%20%C2%B7%20foundation-8A2BE2?style=flat-square)

<!-- ANCHOR: catchphrase -->

## The sovereign, agentic-AI-native messaging server. By Gaussian Technologies.

<!-- ANCHOR_END: catchphrase -->

<!-- ANCHOR: body -->

**GaussMatrix** is a Rust-native, federated communication server engineered for the
era of agentic AI. It is the homeserver half of the GaussMatrix / GaussInteract
platform — a clean-room, enterprise-grade evolution of the
[Matrix](https://matrix.org/) protocol stack designed to outclass the centralised
commercial field (Slack, Microsoft Teams, Discord) on the axes those products
*structurally cannot move*: **data sovereignty, end-to-end encryption, memory
safety, footprint, and a first-class, auditable agentic surface.**

Where the incumbents bolt a cloud assistant onto a closed silo, GaussMatrix treats
AI agents as **governed, cross-signed protocol participants** — scoped, mediated,
E2EE-bound, and recorded in a tamper-evident audit log. An agent admitted to a room
never enlarges that room's trust boundary beyond the humans who admitted it.

> **Built on the shoulders of [Tuwunel](https://github.com/matrix-construct/tuwunel).**
> A companion benchmark selected Tuwunel as the strongest open-source Rust Matrix
> homeserver (aggregate 8.88/10). GaussMatrix adopts Tuwunel's architecture,
> protocol behaviour, and on-disk compatibility as its specification, then hardens
> and extends it toward an eleven-crate, horizontally-scalable, agentic platform.
> Tuwunel is Apache-2.0 licensed; see [`NOTICE`](./NOTICE) for full attribution.

### Why GaussMatrix

| Pillar | What it means |
| --- | --- |
| 🛡️ **Sovereign** | Self-hosted and federated. Your data, your keys, your infrastructure — no third party holds the plaintext, ever. |
| 🔒 **Audited E2EE** | End-to-end encryption delegated entirely to [`vodozemac`](https://github.com/matrix-org/vodozemac) (Olm/Megolm, cross-signing, secure key backup). No hand-rolled cryptography. |
| 🦀 **Memory-safe by design** | Built in Rust with `forbid(unsafe_code)` across the workspace except small, isolated, audited storage/crypto crates. |
| 🤖 **Agentic-native** | A native [Model Context Protocol](https://modelcontextprotocol.io/) gateway with capability scoping, human-in-the-loop approval, E2EE-aware mediation, and a hash-chained audit log. |
| ⚡ **Horizontally scalable** | A room-sharded scaling model that lifts the single-process ceiling of the Conduit lineage — while the same binary collapses to a single-node deployment. |
| 📦 **Operationally mature** | First-party container images, packages (Deb/RPM/Arch/Alpine/Nix), Helm charts, Prometheus metrics, and OpenTelemetry tracing. |

### Architecture at a glance

GaussMatrix is specified as an **eleven-crate Rust workspace** with a pluggable
storage abstraction, a parallelised state-resolution engine, partial-state
federation, and a consistent-hash room-sharding layer. The agentic gateway is a
first-class ingress alongside the Client–Server / Server–Server / Application
Service HTTP surfaces.

```
        Clients      ·      Federation peers      ·      AI agents (MCP)
   ┌──────────────────────────────────────────────────────────────────┐
   │  gm-http (CS/SS/AS)   gm-api (typed model)   gm-agent (MCP gw)     │
   │  gm-svc:  rooms · sync · devices · push · admin                    │
   │  gm-stateres (parallel RV1–12)  gm-fed (partial-state)  gm-e2ee    │
   │  gm-shard: consistent-hash room placement & rebalancing            │
   │  gm-store: pluggable trait · per-domain column families            │
   │     Tuned RocksDB (single-node)   │   Distributed KV (sharded)     │
   └──────────────────────────────────────────────────────────────────┘
```

> **Note on the current tree.** This repository currently contains the adopted
> Tuwunel codebase, rebranded to GaussMatrix and prepared as the Phase-1
> foundation. The eleven-crate `gm-*` decomposition above is the *target*
> architecture delivered incrementally per the [roadmap](#development-roadmap)
> and the full technical specification in
> [`GaussMatrix-SPECS.pdf`](./GaussMatrix-SPECS.pdf).

### Getting started

GaussMatrix runs the full Matrix Client–Server, Server–Server, Application
Service, and push surfaces, and federates with the public Matrix network.

```sh
# Clone
git clone https://github.com/rismanmattotorang/gaussmatrix.git
cd gaussmatrix

# Build (Rust toolchain pinned via rust-toolchain.toml)
cargo build --release

# Configure: copy and edit the generated example config.
# At minimum set `server_name` and `database_path`.
cp gaussmatrix-example.toml gaussmatrix.toml
$EDITOR gaussmatrix.toml

# Run
./target/release/gaussmatrix -c gaussmatrix.toml
```

> ℹ️ The binary is `gaussmatrix`, the workspace crates are `gaussmatrix_*`
> (transitional names; the target eleven-crate `gm-*` decomposition lands in
> Phases 1–4), and the primary config env prefix is `GAUSSMATRIX_`. On-disk and
> protocol compatibility with the Tuwunel/Conduit family is preserved — a Tuwunel
> data directory migrates by binary swap, and the `TUWUNEL_`, `CONDUWUIT_`, and
> `CONDUIT_` config env prefixes are still honoured as migration fallbacks.

See the [documentation](./docs/introduction.md) for deployment guides (Docker,
Podman, Kubernetes, Debian, Arch, NixOS, Red Hat, FreeBSD), reverse-proxy setup,
and configuration reference.

<!-- ANCHOR_END: body -->

## Development roadmap

GaussMatrix is delivered against the four-phase plan in the technical
specification ([`GaussMatrix-SPECS.pdf`](./GaussMatrix-SPECS.pdf), §VII). Each
phase is independently shippable; the linear, documented dependency between phases
preserves auditability.

### Phase 0 — Foundation & rebrand *(complete)*
- [x] Adopt the Tuwunel codebase as the GaussMatrix Phase-1 baseline.
- [x] Rebrand public identity & metadata (README, workspace/package metadata,
      mdBook config, generated configuration) to GaussMatrix / Gaussian Technologies.
- [x] Establish attribution to Tuwunel and upstream lineage ([`NOTICE`](./NOTICE)).
- [x] Rename crates (`tuwunel_* → gaussmatrix_*`), the binary (`tuwunel → gaussmatrix`),
      the config env prefix (`TUWUNEL_ → GAUSSMATRIX_`, old prefixes retained as
      migration fallbacks), and all packaging units (systemd, Deb/RPM/Arch, Podman
      quadlets, install paths).
- [ ] CI supply-chain gates: `cargo audit` + `cargo deny`, reproducible builds.

### Phase 1 — Server core *(drop-in homeserver — in progress)*
- [~] `gm-store` pluggable storage trait with per-domain column families, generalising
      Tuwunel's tuned RocksDB integration. **Landed** (`src/store`): the backend-agnostic
      `KvBackend` trait, the nine-domain column-family model, atomic `WriteBatch` commits,
      the `Store` facade, an in-memory reference backend, and the durable single-node
      **`RocksBackend`** (feature `rocksdb`) opening one column family per domain with
      crash-consistent batch commits — covered by a unit/doctest suite plus
      RocksDB roundtrip and reopen-persistence tests. The service core now **holds a
      backend-agnostic `gm-store::DynStore`** (`Services.store`), opened by
      `store_provider` as a tuned RocksDB engine at `<database_path>/gm-store` for the
      single-node profile. The first consumer — a tamper-evident, hash-chained **`audit`
      service** (`Domain::AuditLog`), where each entry commits to its predecessor's
      SHA-256 and `verify()` detects any retroactive edit (spec §IV-D) — is wired onto it
      end-to-end. Next: more consumers, then the Phase-2 distributed backend.
- [~] `gm-api` typed request/response model (extending `ruma`). **Foundation landed**
      (`src/apimodel`): the event-content adapter layer — parsing
      `m.room.power_levels`/`member`/`join_rules` content (with Matrix defaults and the
      integer-or-string power-level quirk) into the `gm-stateres` models, plus a
      `StateEvent` adapter implementing `gm_stateres::Event` (incl. `from_event_json`
      ingestion of canonical events), and the standard Matrix error model
      (`MatrixError`/`ErrorCode` with errcode + HTTP-status mapping and wire
      serialization), plus a typed endpoint model (`Endpoint`/`Method`/`AuthScope` with
      `{param}` path-template matching) the HTTP ingress dispatches on. Next: the
      `gm_stateres::Event` impl over the server's ruma-backed `Pdu` and more of the CS/SS
      request/response model.
- [ ] Single-node profile with **on-disk compatibility** for drop-in migration from a
      Tuwunel/conduwuit data directory.
- [ ] Full Client–Server / Server–Server conformance against the spec test suite.
- [x] `gm-stateres` state-resolution engine (room versions 1–12) with a
      resolved-state cache. **Landed** (`src/stateres`): the full state-res-v2 two-pass
      `resolve` — conflict partitioning, auth-difference, reverse-topological power
      ordering, mainline ordering, iterative auth checks, and the resolved-state cache —
      plus the room-version authorisation rules (create; power-level send and mutation;
      membership join/invite/leave/kick/ban incl. the create-room bootstrap join, knock,
      and restricted joins) composed via `AllOf`. Pure/deterministic, 44 unit tests incl.
      end-to-end resolution. Remaining: third-party invites and parallel signature
      verification (both require Ed25519 crypto, deferred to the integration layer).

### Phase 2 — Horizontal scale
- [ ] `gm-shard` consistent-hash room placement, coordination, and online rebalancing.
- [ ] Distributed KV storage backend behind the `gm-store` trait.
- [ ] Sharded federation sender (per-destination, no head-of-line blocking) and
      partial-state joins in `gm-fed`.
- [ ] Shared object store for media addressed by content hash.

### Phase 3 — Agentic AI layer
- [ ] `gm-agent` Model Context Protocol gateway (bidirectional Matrix ↔ MCP bridge).
- [ ] Agents as cross-signed Matrix identities provisioned via the Application Service API.
- [ ] Capability scoping (least-privilege grants as versioned room state) with
      `auto` / `review` / `forbidden` action classification.
- [ ] Human-in-the-loop approval surfaced in GaussInteract; E2EE-aware mediation.
- [ ] Tamper-evident, hash-chained audit log in a dedicated storage column family.
- [ ] In-band, namespaced agent events (`m.gauss.agent.tool_call`,
      `m.gauss.agent.tool_result`) for replayable, auditable interactions.

### Phase 4 — Client parity (GaussInteract) & enterprise surface
- [ ] `gauss-core` shared Rust client core (sliding sync, timeline cache, `vodozemac` E2EE).
- [ ] One Flutter presentation layer over `gauss-core` via `uniffi` across Android,
      iOS, Web (WASM), and Linux/macOS/Windows.
- [ ] Agent surface in the client: agent membership, inline tool calls/results,
      approval prompts, read-only audit view.
- [ ] Enterprise features: SSO/OIDC, MDM configuration profiles, enforced key backup
      & cross-signing, UnifiedPush, white-labelling.

### Cross-cutting non-functional targets
*(objectives from the specification, to be validated on the measurement harness — §VIII)*

| Attribute | Target |
| --- | --- |
| Server scaling | Linear horizontal scaling by room shard; single-node mode preserved |
| Server footprint | < 256 MB RSS idle on a single-node small deployment |
| Send latency | p95 local send-to-sync < 150 ms; federation propagation p95 < 800 ms |
| Memory safety | No `unsafe` outside audited, isolated crates; `forbid(unsafe_code)` elsewhere |
| E2EE core | `vodozemac` only; no hand-rolled cryptography |
| Agentic mediation | Agents never bypass room access control or E2EE; every action auditable |
| Supply chain | Reproducible builds; `cargo deny` / `cargo audit` gates in CI |

## Credits & attribution

GaussMatrix is a derivative of **[Tuwunel](https://github.com/matrix-construct/tuwunel)**
(the official successor to [conduwuit](https://github.com/girlbossceo/conduwuit) and
[Conduit](https://gitlab.com/famedly/conduit)), used under the Apache License 2.0.
We gratefully acknowledge the Tuwunel, conduwuit, and Conduit contributors, whose
work forms the architectural and protocol foundation of this project. See
[`NOTICE`](./NOTICE) for the complete attribution and license details.

GaussMatrix also stands on the broader Matrix ecosystem — the
[Matrix.org Foundation](https://matrix.org/), [`ruma`](https://github.com/ruma/ruma),
and [`vodozemac`](https://github.com/matrix-org/vodozemac).

## License

GaussMatrix is licensed under the **Apache License 2.0**. See [`LICENSE`](./LICENSE)
and [`NOTICE`](./NOTICE).

---

<!-- ANCHOR: footer -->

<sub>GaussMatrix is a product of **Gaussian Technologies**, built on the
[Tuwunel](https://github.com/matrix-construct/tuwunel) codebase (Apache-2.0). The
Matrix trademark and specification belong to the Matrix.org Foundation;
GaussMatrix is an independent implementation and is not endorsed by or affiliated
with the Foundation.</sub>

<!-- ANCHOR_END: footer -->

