# Verify Report — m9-04

**Cycle**: m9-04-side-table-key-layout
**Path**: B-direct


## Subject
| Base | Head | Dirty diff digest | CWD | Verified at |
|---|---|---|---|---|
| `eb2cccd` | `379759e` | `sha256:de2fa4a767a4c14090409e68cc63f6ffdf7ddae4ca070c63c46e703366059e1a` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-12T01:21:00+02:00 |

`git status --porcelain` is empty. HEAD is pinned at `379759ed3d5dcc46e84c9b0bc153bab23e360b61`. Subject identity gate: **PASS**.

This is the second verify round. The first round (head `7f86a1c`) returned `FAIL` with 1 critical + 3 warnings + 2 suggestions. The remediation commit `379759e` addressed all 6 findings plus an incidental regression fix. Both rounds used base `eb2cccd`.

## Files Inventory
Source: `git diff --stat eb2cccd..HEAD`.

| Bucket | Added | Modified | Deleted | Renamed |
|---|---:|---:|---:|---:|
| `crates/` | 2 | 5 | 0 | 0 |
| `docs/` | 3 | 0 | 0 | 0 |
| `tests/` | 0 | 0 | 0 | 0 |
| `untagged_project/<segment>` | 0 | 0 | 0 | 0 |

Top paths:
| Status | Bucket | Path | Renamed from | SHA-256 |
|---|---|---|---|---|
| added | crates/ | `crates/chronos-cli/src/lib.rs` | — | n/a |
| added | crates/ | `crates/chronos-cli/tests/replay_integration.rs` | — | n/a |
| added | docs/ | `docs/milestones/m9-04-side-table-key-layout-scoping.md` | — | n/a |
| added | docs/ | `docs/milestones/m9-04-side-table-key-layout-spec.md` | — | n/a |
| added | docs/ | `docs/milestones/m9-04-side-table-key-layout-design.md` | — | n/a |
| modified | crates/ | `crates/chronos-cli/Cargo.toml` | — | n/a |
| modified | crates/ | `crates/chronos-cli/src/replay.rs` | — | n/a |
| modified | crates/ | `crates/chronos-services/src/counterexample.rs` | — | n/a |
| modified | crates/ | `crates/chronos-store/src/counterexample_storage.rs` | — | n/a |
| modified | crates/ | `crates/chronos-store/src/storage.rs` | — | n/a |

Per-crate integration tests now exist for both `chronos-cli` (`tests/replay_integration.rs`, 2 tests) and `chronos-services` (lib `m9_04_save_load_roundtrip_through_services`).

## Summary
| Verdict | Mode | Path | Required scenarios | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|---|
| **PASS** | coordinator | A-min | 9 spec scenarios pinned (6 in store + 1 in services + 2 in cli replay) | T0, T1, T2, T4-smoke all green | 0 | 0 |

## Remediation Summary

| Previous finding | Severity | Status | Evidence |
|---|---|---|---|
| `task-completeness-missing-integration-tests` (3 spec-promised tests) | critical | **RESOLVED** | `m9_04_save_load_roundtrip_through_services` (services line 3537), `m9_04_replay_uses_v3_layout` (cli replay_integration line 25), `m9_04_replay_v2_bundle_uses_legacy_path` (cli replay_integration line 126) |
| `test-quality-range-bound-tautology` | medium | **RESOLVED** | `m9_04_range_scan_covers_all_chunk_indices` rewritten: inserts A=4 + B=10 chunks via direct injection, asserts only A's 4 chunks are returned by `load_counterexample_bundle_events(A)` and only B's 10 chunks by `load_counterexample_bundle_events(B)`. Range bound is now exercised end-to-end through the public API |
| `test-quality-collision-not-forced` | medium | **RESOLVED** | `__force_collision_prefix` helper bypasses blake3 to write a chosen 16-byte prefix; `m9_04_forced_prefix_collision_is_contained_by_identity_check` forces a true key-prefix collision between two bundle IDs and verifies the per-chunk identity filter drops the foreign chunk |
| `spec-divergence-d7-events-count-guard` | low | **RESOLVED** | `collect_bundle_chunks` now takes `events_count: Option<u64>`; `SessionStore::get_bundle_events_count` helper reads `summary.events_count` from the bundle record; guard returns empty when `Some(0)` is observed; `None` (direct-injection test path) preserves existing coverage |
| `disclosure-r4-no-test-pinning` | low | **RESOLVED** | `m9_04_r4_v2_bundle_never_resaved_uses_legacy_path`: injects v2 chunk + bundle record (events_count = 1) directly, reads twice, asserts both reads return identical events via fallback path |
| `disclosure-r6-no-test-pinning` | low | **RESOLVED** | `m9_04_known_bundle_schema_versions_pinned`: asserts `KNOWN_BUNDLE_SCHEMA_VERSIONS` contains `[1, 2, 3]` with `len == 3` |
| Incidental regression: `project_report` read `bundle.events.len()` | medium | **FIXED** | `crates/chronos-cli/src/replay.rs:243` now uses `bundle.summary.events_count as usize`. Pre-fix, post-m9-02 bundles reported `events_in_bundle = 0` because events live in the side table. The CLI integration test (`m9_04_replay_uses_v3_layout` line 106-108) asserts `report.events_in_bundle == 5` after save+replay, locking the fix |

## Behavioral Compliance

| Requirement / Scenario | Production Path | Test | Status | Evidence |
|---|---|---|---|---|
| Side-table keys use fixed-width blake3 prefix (v3) | `encode_chunk_key`; `bundle_prefix` | store unit tests + cli + services | **COMPLIANT** | T1 store 56/56; T2 cli 2/2; T2 services 1/1 |
| Side-table values carry `bundle_id` | `encode_chunk_value` / `decode_chunk_value`; identity filter | store unit tests | **COMPLIANT** | T1 store 56/56 |
| Read path: v3 range scan first, v2 fallback | `collect_bundle_chunks` (D7 guard: skip v2 when `Some(0)`) | store unit + cli integration | **COMPLIANT** | T1 store 56/56; T2 cli 2/2 |
| Save writes v3 + removes both v3 and v2 prior chunks | `save_bundle_record_and_events` (dual cleanup) | store unit | **COMPLIANT** | T1 store 56/56 |
| `CURRENT_BUNDLE_SCHEMA_VERSION == 3`; future-version rejected | store constant + future-version test | store unit + KNOWN pin | **COMPLIANT** | T1 store 56/56 (m9_04_known_bundle_schema_versions_pinned) |
| Save → load round-trip under v3 layout | store `save_counterexample_bundle` / `load_counterexample_bundle_events` | store unit + cli + services | **COMPLIANT** | T1 store 56/56; T2 cli 2/2; T2 services 1/1 |
| **Services + replay chokepoint under v3 + v2 (services)** | `ChronosCounterexampleService::save` → `bundle_events_or_legacy` | `m9_04_save_load_roundtrip_through_services` | **COMPLIANT** | T2 services 1/1 (line 3537) |
| **Replay uses v3 layout (cli)** | `chronos_cli::replay::run_replay` | `m9_04_replay_uses_v3_layout` | **COMPLIANT** | T2 cli 1/1 (line 25); also asserts `report.events_in_bundle == 5` to lock the project_report fix |
| **Replay uses v2 fallback for legacy bundle (cli)** | `run_replay` → `bundle_events_or_legacy` → v2 branch | `m9_04_replay_v2_bundle_uses_legacy_path` | **COMPLIANT** | T2 cli 1/1 (line 126) |
| Range scan is bounded to one bundle's chunks | `collect_bundle_chunks_range` `[prefix||0, prefix||u32::MAX)` | `m9_04_range_scan_covers_all_chunk_indices` (end-to-end with A+B) | **COMPLIANT** | T1 store 56/56 |
| Hash-prefix collision containment via identity check | value-identity filter | `m9_04_collision_containment_via_bundle_id` + `m9_04_forced_prefix_collision_is_contained_by_identity_check` | **COMPLIANT** | T1 store 56/56 |
| `KNOWN_BUNDLE_SCHEMA_VERSIONS == [1, 2, 3]` (R6 pin) | store constant | `m9_04_known_bundle_schema_versions_pinned` | **COMPLIANT** | T1 store 56/56 |
| Never-resaved v2 bundle keeps using legacy path (R4 pin) | `collect_bundle_chunks_legacy` | `m9_04_r4_v2_bundle_never_resaved_uses_legacy_path` | **COMPLIANT** | T1 store 56/56 |

All 9 spec-required scenarios are pinned by tests that exercise production code paths end-to-end.

## Production Readiness
| Gate | Status | Evidence | Findings / N/A reason |
|---|---|---|---|
| Errors / recovery | PASS | `StoreError` enum unchanged; legacy reader returns `Ok(Vec::new())` for missing tables; D7 guard returns empty on `Some(0)` events_count | — |
| State / data integrity | PASS | Atomic write tx removes both v3 and v2 prior chunks; bundle record and side-table updates are linked through `save_bundle_record_and_events` | — |
| Resource cleanup | PASS | `read_tx` opened and dropped inside scope; `write_tx.commit()` after writes; no leaked handles across the 13 m9_04 store unit tests | — |
| Concurrency | PASS | redb transactions own their tables; no new shared state | — |
| Migrations / compatibility | PASS | v2 rows readable via fallback; R3 documents one full-table scan on save; R4 documented and pinned; D7 guard prevents spurious v2 fallback for empty post-m9-02 bundles | — |
| Security | N/A | No new external input / auth / secrets | — |
| Performance | PASS | Range scan bound to one bundle's chunks; O(chunks) hot path; D7 guard short-circuits empty events_count cases | — |
| Observability / deployability | PASS | tracing unchanged; same module doc gains "Key layout (m9-04)" section (R7) | — |

## Code Quality

| Standard | Status | Evidence | Findings |
|----------|--------|----------|----------|
| Business code reality (no stub / mock / hardcoded satisfier in `src/` / `lib/` / `bin/`) | PASS | grep for `TODO\|FIXME\|XXX\|HACK\|todo!\|unimplemented!\|panic!\|unreachable!` in changed production paths (`crates/chronos-store/src/counterexample_storage.rs`, `crates/chronos-cli/src/replay.rs`, `crates/chronos-cli/src/lib.rs`, `crates/chronos-services/src/counterexample.rs`, `crates/chronos-store/src/storage.rs`) returns 0 hits in production paths. `__force_collision_prefix` is a test-only helper inside the `tests` module and is `debug_assert_eq!`-bounded; production code never calls it | — |
| Documentation discipline (no issue / task / user / cycle refs in comments) | PASS | All `m9-04 / m9-02 / D1..D7 / R2/R4/R6` references in comments are design-decision anchors from the cycle's own scoping/spec, paired with behavioral explanations per the "primary content explains behavior, may attach requirement ID" allowance | — |

## SOLID And Design
| Principle / Decision | Status | Concrete evidence | Impact |
|---|---|---|---|
| SRP | PASS | Two concerns in this cycle: (a) side-table v3 layout (store), (b) chokepoint integration at the consumer layer (services + cli). Both are local and well-scoped | Local |
| OCP | PASS | Stable public API (`save_counterexample_bundle`, `load_counterexample_bundle_events`, `count_counterexample_bundle_events`, `bundle_events_or_legacy`); extension via new helpers (`collect_bundle_chunks_range`, `_legacy`, `bundle_prefix`, `encode_chunk_value`, `decode_chunk_value`, `get_bundle_events_count`); `SessionStore::db()` visibility widened from `pub(crate)` to `pub` to enable cli integration tests without breaking encapsulation (typed table accessors still preferred) | None |
| LSP | PASS | All read paths preserve `(Vec<TraceEvent>, StoreError)` contract; legacy fallback returns same shape; `events_count: Option<u64>` is a backwards-compatible parameter addition (was `()`) | None |
| ISP | PASS | `SessionStore` API surface unchanged for non-test callers; new `db()` accessor documented as "minimum visibility" | None |
| DIP | PASS | No new infrastructure dep introduced; `bincode` was already in `Cargo.toml` (added to chronos-cli workspace member for test-only use) | None |

| Design decision | Status | Notes |
|---|---|---|
| D1 (16-byte blake3 prefix) | PASS | `bundle_prefix`; cost ≈ 50 ns per hash |
| D2 (keep legacy reader) | PASS | `collect_bundle_chunks_legacy` |
| D3 (value carries bundle_id) | PASS | `encode_chunk_value` / `decode_chunk_value` |
| D4 (range-scan upper bound) | PASS | `[prefix||0, prefix||0xFFFFFFFF)` |
| D5 (schema_version bump) | PASS | `CURRENT_BUNDLE_SCHEMA_VERSION = 3`; future-version rejected |
| D6 (atomic save: v3 + v2 cleanup) | PASS | `save_bundle_record_and_events` |
| D7 (read: v3 first, v2 second, `events_count > 0` guard) | PASS | `collect_bundle_chunks(tx, id, events_count)`; `events_count == 0` → skip v2 fallback; `None` → always try v2 (direct-injection test path); `Some(n>0)` → try v2 when v3 returns empty |
| D8 (identity check per chunk) | PASS | `__force_collision_prefix` helper + `m9_04_forced_prefix_collision_is_contained_by_identity_check` proves prefix-collision containment end-to-end |
| D9 (naming: keep `encode_chunk_key` for v3) | PASS | Renames + new helpers per design table |

## Incidental Regression Fix

`crates/chronos-cli/src/replay.rs:243` previously computed `events_in_bundle: bundle.events.len()`. After m9-02, events live in the side table (`counterexample_bundle_events`), and `bundle.events` is empty for any post-m9-02 bundle — so the `ReplayReport.events_in_bundle` field was always 0 in production. The remediation commit fixed it to `bundle.summary.events_count as usize`, which is the persistent source of truth (set by `save_counterexample_bundle` to `events.len()` at save time). The CLI integration test `m9_04_replay_uses_v3_layout` asserts `report.events_in_bundle == 5` (line 106-108) after a save+replay round-trip, locking the fix.

This was an actual production regression: any `chronos test replay` invocation against a post-m9-02 bundle reported `events_in_bundle: 0` despite loading and replaying the events correctly. The bug had been latent because the integration test for replay (CLI integration suite) did not exist before this round — the per-crate integration tests added by the remediation commit close that gap.

## Architecture Delta
Architecture manifest is **not_applicable** (local store-layer impact; no external boundary; no deployment change). The `bundle_events_or_legacy` chokepoint (m9-02 D5) is preserved verbatim — the v3/v2 branching is absorbed inside `load_counterexample_bundle_events`, invisible to consumers in `chronos-services`, `chronos-cli`, `chronos-mcp`.

## Commands
| Command | Exit | Subject | Evidence |
|---|---:|---|---|
| `cargo fmt --all -- --check` (CARGO_TARGET_DIR=/var/home/rubentxu/cargo-targets) | 0 | `379759e` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | `379759e` | clean |
| `cargo test --workspace --lib --no-fail-fast --exclude chronos-native --exclude chronos-ebpf` | 0 | `379759e` | 870+ tests across 15 crates, 0 failed. Per-crate counts: 42 + 26 + 11 + 149 + 24 + 8 + 44 + 13 + 48 + 76 + 30 + 77 + 3 + 263 + 56. chronos-native / chronos-ebpf excluded due to known pre-existing ptrace flake (AGENTS.md §6.5) |
| `cargo test -p chronos-cli -p chronos-services -p chronos-store --tests --no-fail-fast` | 0 | `379759e` | chronos-cli lib 11 + chronos-cli integration 22 (incl. m9_04_replay_uses_v3_layout + m9_04_replay_v2_bundle_uses_legacy_path); chronos-services lib 263 (incl. m9_04_save_load_roundtrip_through_services); chronos-services integration 2; chronos-store lib 56 (incl. m9_04_known_bundle_schema_versions_pinned, m9_04_r4_v2_bundle_never_resaved_uses_legacy_path, m9_04_range_scan_covers_all_chunk_indices, m9_04_forced_prefix_collision_is_contained_by_identity_check); chronos-store integration 0 |
| `cargo clean -p chronos-mcp && cargo build --bin chronos-mcp` | 0 | `379759e` | Fresh binary at `/var/home/rubentxu/cargo-targets/debug/chronos-mcp` (138 MB, mtime 2026-09-12 01:21, > remediation commit mtime 00:54) |
| `cargo test -p chronos-sandbox --test counterexample_tools --test e2e_connectivity` (CHRONOS_MCP_PATH set) | 0 | `379759e` | counterexample_tools 12/12 (ce1..ce12 incl. ce12_replay_preserves_non_default_invariant_options which exercises replay chokepoint through MCP); e2e_connectivity 1/1 (test_mcp_server_starts_and_responds) |

T4-smoke subset rationale (per AGENTS.md §2): cycle is local store-layer + thin CLI/services chokepoints, but the MCP server is the production entry point. `counterexample_tools` exercises the full save→list→get→replay round-trip through MCP (12 cases including ce12 replay-options preservation); `e2e_connectivity` confirms server starts and responds. Both depend on the v3/v2 side-table chokepoint being correct end-to-end.

## Issues

### CRITICAL
None.

### WARNING
None.

### SUGGESTION
None.

## Lens Summary
| Lens | Findings | Evidence gaps |
|---|---|---|
| `spec-compliance` (A-min default) | 0 — all 9 spec scenarios pinned | none |
| `test-quality` (A-min default) | 0 — range-bound and forced-prefix-collision tests are now real end-to-end checks | none |

## Verdict

**PASS**

Reason tied to mandatory gates:
- `subject_identity`: PASS — clean tree, HEAD pinned at `379759e`, diff digest computed.
- `behavioral_compliance`: PASS — all 9 spec scenarios pinned by tests that exercise production code paths end-to-end through the v3/v2 chokepoint (services wrapper + CLI replay).
- `real_implementation`: PASS — no stubs, mocks, hardcoded satisfiers, or unreachable bodies in changed production paths.
- `documentation_discipline`: PASS — no traceability-only comments; `m9-04/D1..D7/R2/R4/R6` markers pair with behavioral explanations.
- `test_strength`: PASS — range-bound test now inserts two bundles and counts; collision test forces a real prefix collision and verifies the identity filter drops the foreign chunk; R4 test reads the same v2 bundle twice; KNOWN pin is direct.
- `regression_and_build`: PASS — T0 fmt+clippy clean; T1 870+ lib tests across 15 crates; T2 cli/services/store integration tests; T4-smoke 13 sandbox tests with fresh binary.
- `production_readiness`: PASS — D7 guard prevents spurious v2 fallback for empty post-m9-02 bundles; R4 unresaved-v2 invariant is tested; security / concurrency / migrations unchanged from prior cycle and confirmed by store unit suite.
- `design_and_solid`: PASS — D7 guard aligns implementation with scoping step 3; chokepoint preserved; one incidental real regression (project_report reading `bundle.events.len()`) discovered and fixed with a regression test in cli integration suite.
- `task_completeness`: PASS — all 6 prior findings resolved; per-crate integration tests added for both chronos-cli and chronos-services (3 tests total: 2 cli + 1 services).

The cycle is ready to advance to `sddk-debt-verify` on the A-min path. (Per the launch prompt, this verify executor does NOT execute sddk cycle transitions; the orchestrator should drive the lifecycle.)
## Cross-checks

Note: This cycle predates the cross-check annotation format introduced
in m9-28. Per `vault-drift-sweep.md` cross-check #21 (verify-report
must have `## Cross-checks` section), this section is added
retrospectively by m9-32. The cycle's verify-report content above is
unchanged.

The cross-check status for this cycle was inferred from the
apply-checkpoint.json status field:
- Status: CLOSED (verified, released, archived)
- All apply-checkpoint.json SHA fields match git repository
- No drift detected when this cycle was authored
