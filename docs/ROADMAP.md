# Chronos roadmap

> **Status:** reconstruction convergence phase. See `docs/chronos-agentic-reconstruction/`
> for the target architecture, compliance ledger, convergence gates and official milestones.

## Active milestone

**REC-C4 — SOLID capability split + connascence reduction — Status: CLOSED (REC-C4 archive landed 2026-09-20; ratchet at zero, zero waivers)**

**REC-C5 — Canonical Agent API convergence — Status: CLOSED (REC-C5 C5.3.2 landed 2026-09-21; wire surface 63 → 41 tools, ALL_TOOL_NAMES synced with `#[tool]` router; alias_deletion regression test green)**

Next active gate is **REC-C6 — Close unfinished M1–M4 reconstruction contracts**.

REC-C1 and REC-C2 are both CLOSED on `main`:

- REC-C1.5 closure at `a1a79c80` (tag `rec-c1.5-closure`)
- REC-C1.6 lifecycle-safe delete + retention/tail on wire at `83ee38e2` (tag `rec-c1-6-lifecycle-retention-wire`)
- REC-C1.7 projection authority acceptance at `6190390d` (tag `rec-c1-7-projection-authority-acceptance`)
- REC-C1.8 authoritative evidence handoff at `451a29b6` (tag `rec-c1-8-authoritative-evidence-handoff`) — REC-C1 fully closed
- REC-C2.0 eventbus legacy inventory + ratchet at `4c7df70e`
- REC-C2.1 TripwireFired as ExecutionLog evidence at `686a364c`
- REC-C2.2 accepted-Raw seam + producer convergence at `f02ab311` (tag `rec-c2.2-accepted-raw-seam`)
- REC-C2.3 EventBus removal at `d435557e` (tag `rec-c2.3-eventbus-removal`)
- REC-C2.5 formal closure at `<pending merge>` (tag `rec-c2-5-formal-closure`) — REC-C2 fully closed

Next gate is **REC-C3 — Hexagonal boundary closure**, owned by the
`HEX-*` and `CONN-*` contracts in `reconstruction-contracts.toml`.
REC-C3 is multi-cycle (C3.1..C3.5 per
`docs/chronos-agentic-reconstruction/docs/roadmap/CONVERGENCE_BACKLOG.md`):
application ports, extract webhook infrastructure, invert
services -> concrete adapter dependencies, remove store -> native,
dependency graph gate to zero.

**REC-C3.1 (closed 2026-09-18, tag `v0.1.1`):** `crates/chronos-domain/src/ports/{execution_log,notification,probe,session,telemetry}.rs` declared; `chronos_domain::session_id::SessionId` introduced to break the cyclic dep with `chronos_log`. `scripts/check_hex_boundary.py` enforces ports/* outbound purity + surface shape. HEX-001 moved `gap -> partial`. `ExecutionLogProvider` is a documented placeholder for C3.3.

**REC-C3.2 (closed 2026-09-19):** `scripts/check_hex_boundary.py` hardened. The gate now enforces (a) chronos-domain `Cargo.toml` blacklist (no reqwest/hyper/tokio/tracing/axum/warp/http and no other workspace `chronos_*` infra crate), (b) full `crates/chronos-domain/src/**/*.rs` outbound purity (no forbidden external crates — type-only deps and intrinsics are whitelisted), (c) chronos-webhook adapter direction (`chronos-webhook -> chronos_domain` only — services-side is C3.3), and (d) `ports/mod.rs` public surface shape with stale-waiver detection. No waiver list is configured (zero waivers is the target; REC-C3.5 will keep that property). HEX-001 promotes `partial -> verified`; HEX-C32-01/02/03 are added as `verified` (V5 direction settled). chronos-webhook remains the single home of reqwest — that is unchanged by this cycle; C3.2 reconciles the contract/gate because the code was already in position from C3.1. HEX-002 stays `gap`; the services-side inversion is C3.3.

**REC-C3.3 / C3.4 (closed 2026-09-19/20):** services-side inversion executed. `SessionReader`/`SessionArchive`/`LifecycleStore`/`CounterexampleRepository`/`ExecutionLog` ports consumed by services; redb-backed adapters (`session_reader_adapter`, `session_archive`, `lifecycle_store_adapter`, `counterexample_repository`) wired at the `chronos_mcp::composition` root. `chronos-services` stopped naming `SessionStore` in production code.

**REC-C3.5 — residual inversion (closed 2026-09-20, branch `feat/rec-c3.5-residual-inversion`):** closed the three residual production edges and emptied the architecture debt baseline:
- **R.1** (`110a47e4`): dropped the stale `chronos-store -> chronos-native` waiver (edge already gone after C3.3.4).
- **R.2** (`8e8d7b48`): `DiffEngine` port in `chronos_domain::ports::diff` + `Blake3DiffEngine` adapter in chronos-store; `chronos_store::diff::TraceDiff` is a `#[deprecated]` delegating wrapper. Services consume `Arc<dyn DiffEngine>` via `DiffContext`/`SessionCompareContext`.
- **R.3** (`2a8d5b8b`): `NativeProbeControllerFactory` port (`build_for_spawn`/`build_for_attach`) + `ChronosNativeProbeControllerFactory` impl; `ProbeService` no longer names `NativeProbeBackend`/`NativeProbeControllerImpl`; `chronos-native` moved to services `[dev-dependencies]`. Audit §4.5 S4 composition leak closed.
- **R.4** (counterexample wire types moved to `chronos_domain::ports::counterexample::wire`; store re-exports for the redb path): `MinimisedPayload`, `ExistencePredicateWire`, `HypothesisInputWire` + domain-owned `CURRENT_BUNDLE_SCHEMA_VERSION` with a compile-time drift guard in `ce_schema`.
- **R.5** (`b7a55d9e`): `chronos-store` moved to services `[dev-dependencies]`; `known_dependency_violations = []`. `check_architecture_contracts.py` reports zero forbidden edges, zero waivers; `check_hex_boundary.py` clean.

REC-C3 hexagonal closure objective met: the application layer (chronos-services) consumes only `chronos_domain::ports::*`; every concrete backend is wired at the composition root (`chronos_mcp::composition`). Known environmental flake observed during close: `test_get_execution_summary_busyloop` fails when analytics_tools runs immediately after e2e_connectivity under system load (pass in isolation and 3/3 in back-to-back suite runs) — tracked for AGENTS.md §6.5 if it reproduces at release.

The `## Convergence sequence` table below reflects this state.

## Convergence sequence

1. **REC-C0 — Restore the truth baseline** — CLOSED (C0.5-D sentinel)
2. **REC-C1 — ExecutionLog cutover and truthful reads** — CLOSED
3. **REC-C2 — Legacy evidence/event-path deletion** — CLOSED
4. **REC-C3 — Hexagonal boundary closure** — CLOSED (2026-09-20, `feat/rec-c3.5-residual-inversion`)
5. **REC-C4 — SOLID + connascence reduction** — CLOSED (2026-09-20, tag `rec-c4-archive`)
6. **REC-C5 — Canonical Agent API convergence** — CLOSED (2026-09-21, `feat/rec-c5-api-convergence`, wire surface 63 → 41 tools)
7. **REC-C6 — Close unfinished M1–M4 reconstruction contracts** — NEXT (active gate)
8. **REC-C7 — Reconstruction convergence close**

Only REC-C7 can unblock official reconstruction **M6 OpenTelemetry correlation + export**.

## Closed historical delivery cycles

The following cycle/milestone records remain historically closed. Their close state does **not** automatically mean that every reconstruction requirement is currently verified; residual obligations are tracked in `reconstruction-contracts.toml` and owned by REC-C gates.

- **m0-truth-first-foundation** — closed
- **m5-agent-api-v2** — closed 2026-09-10
- **m6-v2-spec-surface-reduction** — closed 2026-09-11
  - internal API sub-cycle; distinct from official reconstruction M6.
- **m7-v2-spec-introspection** — closed 2026-09-11
  - internal API sub-cycle; distinct from official reconstruction M7.
- **m8-counterexample-shrinking** — closed 2026-09-11
  - foundation delivered; official roadmap M8 still requires end-to-end test-intelligence completion.
- **m9-vault-hygiene** — closed 2026-09-14
  - repository governance/vault hygiene; distinct from official reconstruction M9.
- **m10-vault-ms-cleanup** — closed 2026-09-15
  - repository governance namespace; distinct from official reconstruction M10 Execution Explorer.
- **rec-c1-5-closure** — closed 2026-09-17 (merged --no-ff into `main` as `a1a79c80`; tag `rec-c1.5-closure`).
  - canonical ExecutionLog root resolver, MCP startup bootstrap, durable delete, durable seal on clean stop. Four real-process sandbox UATs (R1..R4) plus the readiness invariant.
- **rec-c1-6-lifecycle-retention-wire** — closed (tag `rec-c1-6-lifecycle-retention-wire` at `83ee38e2`). Lifecycle-safe `delete_session` + `RetentionFacts`/`TailFacts` on `events_read` wire + `CursorStale` envelope with structured `requested_next_seq`/`retained_from_seq`.
- **rec-c1-7-projection-authority-acceptance** — closed (tag at `6190390d`). `chronos_services::projection::build_engine` + projection gate in MCP handlers. UAT-REC-C1-01 (two consumers), UAT-REC-C1-05 (time semantics), restart-equivalence on the wire.
- **rec-c1-8-authoritative-evidence-handoff** — closed (tag at `451a29b6`). Three acceptance discrepancies from C1.7 closed; `chronos_log::segmented` bookkeeping bug surfaced as `FIND-C1.8-01` (deferred). REC-C1 fully closed.
- **rec-c2-eventbus-removal** (C2.0..C2.3) — closed (C2.3 tag `rec-c2.3-eventbus-removal` at `d435557e`). `chronos-domain::bus` deleted; `ProbeBackend::read_since` removed; `bus_capacity`/`bus_fill` removed from wire; ratchet at baseline 0.
- **rec-c2-5-formal-closure** — closed (tag `rec-c2-5-formal-closure`, see `cycle-artifacts/p-3416cfb8288f8964/rec-c2-5-formal-closure/`). LEGACY-001/002 contracts flipped to `verified`; `reconstruction-contracts.toml` `active_gate` flipped from `REC-C2` to `REC-C3`. REC-C2 fully closed.
- **rec-c3-1-application-ports** — closed 2026-09-18 (tag `v0.1.1` at `33b4f790`). HEX-001 promoted `gap -> partial`; `ExecutionLogProvider` is a documented placeholder; `session_id::SessionId` introduced to break the cyclic dep with `chronos_log`. REC-C3.1 is the foundation; REC-C3.2 reconciles the gate.

## Naming rule

From this point forward:

- `REC-C*` = reconstruction convergence gates;
- `M*` = official product reconstruction milestones;
- governance/vault/housekeeping cycles must use a non-product prefix and must not be presented as completion of an official `M*` milestone.

This removes the previous ambiguity where an internal `m10-*` cycle could be confused with M10 Execution Explorer.

## Specification truth

Machine-readable current compliance:

`/reconstruction-contracts.toml`

Architecture/spec fitness gate:

```bash
python3 scripts/check_architecture_contracts.py
```

REC-C7 strict close:

```bash
python3 scripts/check_architecture_contracts.py --strict-no-gaps
cargo check --workspace --all-targets --all-features
cargo test --workspace -- --test-threads=1
```

## Official future milestones after convergence

1. **M6 — OpenTelemetry correlation + export**
2. **M7 — Differential execution v2**
3. **M8 — Counterexample shrinking and test intelligence**
4. **M9 — Concurrency intelligence / happens-before**
5. **M10 — Execution Explorer**
6. **M11 — Additional language depth**

See `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` for detailed scope and gates.

## Cycle serialization lock

Only one product/convergence milestone is `Status: in_progress` at a time. The owning cycle must complete, be explicitly blocked, or be abandoned before another product/convergence milestone is promoted. Repository housekeeping may run independently only when it cannot alter product-delivery claims or acceptance evidence.
