# Verification Report: m9-04-side-table-key-layout

## Subject
| Base | Head | Dirty diff digest | CWD | Verified at |
|---|---|---|---|---|
| `eb2cccd` | `7f86a1c` | `sha256:388fb903a615155c1a3aadba435ab234a3ad5d28237ba9e723c5c58ba85f8eb7` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-11T23:00:00+02:00 |

`git status --porcelain` is empty. HEAD is pinned at `7f86a1c565a2343748b45a3bc906729208590b38`. Subject identity gate: **PASS**.

## Files Inventory
Source: `git diff --stat eb2cccd..HEAD`.

| Bucket | Added | Modified | Deleted | Renamed |
|---|---:|---:|---:|---:|
| `crates/` | 0 | 1 | 0 | 0 |
| `docs/` | 3 | 0 | 0 | 0 |
| `tests/` | 0 | 0 | 0 | 0 |
| `untagged_project/<segment>` | 0 | 0 | 0 | 0 |

Top paths:
| Status | Bucket | Path | Renamed from | SHA-256 |
|---|---|---|---|---|
| modified | crates/ | `crates/chronos-store/src/counterexample_storage.rs` | — | n/a |
| added | docs/ | `docs/milestones/m9-04-side-table-key-layout-scoping.md` | — | n/a |
| added | docs/ | `docs/milestones/m9-04-side-table-key-layout-spec.md` | — | n/a |
| added | docs/ | `docs/milestones/m9-04-side-table-key-layout-design.md` | — | n/a |

Note: the apply commit touches ONLY `crates/chronos-store/src/counterexample_storage.rs`. No `crates/<c>/tests/*.rs` integration test files were added or modified despite the spec promising 3 per-crate integration tests.

## Summary
| Verdict | Mode | Path | Required scenarios | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|---|
| **FAIL** | coordinator | A-min | 6 spec scenarios pinned, 3 spec scenarios UNPINNED | T0, T1, T2, T4-smoke all green | 1 | 4 |

## Behavioral Compliance

| Requirement / Scenario | Production Path | Test | Status | Evidence |
|---|---|---|---|---|
| Side-table keys use fixed-width blake3 prefix (v3) | `encode_chunk_key` lines 120-127; `bundle_prefix` lines 104-113 | `m9_04_v3_key_layout_fixed_width`, `m9_04_bundle_prefix_is_16_bytes`, `m9_04_decode_chunk_key_roundtrip`, `m9_04_decode_chunk_key_rejects_wrong_length` | **COMPLIANT** | T1 53/53 pass |
| Side-table values carry `bundle_id` for identity verification | `encode_chunk_value` lines 146-148; `decode_chunk_value` lines 153-155; identity filter in `collect_bundle_chunks_range` lines 236-242 | `m9_04_encode_decode_value_carries_bundle_id`, `m9_04_collision_containment_via_bundle_id` | **COMPLIANT** | T1 53/53 pass |
| Read path: v3 range scan first, v2 fallback | `collect_bundle_chunks` lines 282-292; `collect_bundle_chunks_range` lines 209-247; `collect_bundle_chunks_legacy` lines 255-274 | `m9_04_v2_bundle_loads_via_fallback`, `m9_04_resave_migrates_v2_to_v3`, `m9_04_fresh_store_ghost_bundle_returns_empty`, `m9_04_v2_bundle_with_zero_events_count_loads_normally` | **COMPLIANT** (in-tree) | T1 53/53 pass |
| Save writes v3 + removes both v3 and v2 prior chunks | `save_bundle_record_and_events` lines 503-597 (dual cleanup at 530-565) | `m9_04_resave_migrates_v2_to_v3` | **COMPLIANT** | T1 53/53 pass |
| `CURRENT_BUNDLE_SCHEMA_VERSION == 3`; future-version rejected | line 294 + lines 704-710 | `m9_04_saved_v3_schema_and_future_rejection` | **COMPLIANT** | T1 53/53 pass |
| Save → load round-trip under v3 layout | `save_counterexample_bundle` 463-490 + `load_counterexample_bundle_events` 607-636 | `m9_04_v3_save_load_roundtrip` | **COMPLIANT** | T1 53/53 pass |
| Services + replay chokepoint under v3 + v2 (services) | `bundle_events_or_legacy` lines 815-825 (chokepoint preserved); `ChronosCounterexampleService::save` | **MISSING: `m9_04_save_load_roundtrip_through_services`** | **UNTESTED** | spec §5 required test not delivered |
| Replay uses v3 layout (cli) | `chronos_cli::replay::run_replay` | **MISSING: `m9_04_replay_uses_v3_layout`** | **UNTESTED** | spec §5 required test not delivered |
| Replay uses v2 fallback for legacy bundle (cli) | same | **MISSING: `m9_04_replay_v2_bundle_uses_legacy_path`** | **UNTESTED** | spec §5 required test not delivered |

The lib-level contract is fully pinned by 13 m9_04 unit tests. The chokepoint integration contract (services+cli) is NOT pinned.

## Production Readiness
| Gate | Status | Evidence | Findings / N/A reason |
|---|---|---|---|
| Errors / recovery | PASS | `StoreError` enum unchanged; legacy reader returns `Ok(Vec::new())` for missing tables | — |
| State / data integrity | PASS | Atomic write tx removes both v3 and v2 prior before writing new; verified by `m9_04_resave_migrates_v2_to_v3` | — |
| Resource cleanup | PASS | `read_tx` opened and dropped inside scope; `write_tx.commit()` after writes | — |
| Concurrency | PASS | redb transactions own their tables; no new shared state | — |
| Migrations / compatibility | PASS (with note) | v2 rows readable via fallback; R3 documents one full-table scan on save; R4 documents v2-only bundles keep legacy path | `findings[].disclosure-r4-no-test-pinning` (suggestion) |
| Security | N/A | No new external input / auth / secrets | — |
| Performance | PASS | Range scan bound to one bundle's chunks; O(chunks) hot path; documented R2 collision risk | `findings[].test-quality-range-bound-tautology` (warning) |
| Observability / deployability | PASS | tracing unchanged; same module doc gains "Key layout (m9-04)" section (R7) | — |

## Code Quality

| Standard | Status | Evidence | Findings |
|----------|--------|----------|---------|
| Business code reality (no stub / mock / hardcoded satisfier in `src/` / `lib/` / `bin/`) | PASS | grep for `TODO\|FIXME\|XXX\|HACK\|todo!\|unimplemented!\|panic!\|unreachable!` in `crates/chronos-store/src/counterexample_storage.rs` returns 0 hits in production paths (only `debug_assert_eq!` and `.unwrap()` on infallible bincode of `(String, Vec<TraceEvent>)`) | — |
| Documentation discipline (no issue / task / user / cycle refs in comments) | PASS (with caveat) | All `m9-04 / m9-02 / D1..D7` references are design-decision anchors from the cycle's own scoping/spec, paired with behavioral explanations per the "primary content explains behavior, may attach requirement ID" allowance. Section markers (`// m9-04 v3 key/value encoding`) are organizational but always carry descriptive text | — |

## SOLID And Design
| Principle / Decision | Status | Concrete evidence | Impact |
|---|---|---|---|
| SRP | PASS | One file modified (`counterexample_storage.rs`); one concern (side-table layout) | Local |
| OCP | PASS | Stable public API (`save_counterexample_bundle`, `load_counterexample_bundle_events`, `count_counterexample_bundle_events`, `bundle_events_or_legacy`); extension via new helpers (`collect_bundle_chunks_range`, `_legacy`, `bundle_prefix`, `encode_chunk_value`, `decode_chunk_value`) | None |
| LSP | PASS | All read paths preserve `(Vec<TraceEvent>, StoreError)` contract; legacy fallback returns same shape | None |
| ISP | PASS | `SessionStore` API surface unchanged | None |
| DIP | PASS | No new infrastructure dep introduced; blake3 was already in `Cargo.toml` | None |

| Design decision | Status | Notes |
|---|---|---|
| D1 (16-byte blake3 prefix) | PASS | Implemented `bundle_prefix` lines 104-113; cost ≈ 50 ns per hash |
| D2 (keep legacy reader) | PASS | `collect_bundle_chunks_legacy` lines 255-274 |
| D3 (value carries bundle_id) | PASS | `encode_chunk_value` / `decode_chunk_value` lines 146-155 |
| D4 (range-scan upper bound) | PASS | `[prefix||0, prefix||0xFFFFFFFF)` lines 219-228 |
| D5 (schema_version bump) | PASS | `CURRENT_BUNDLE_SCHEMA_VERSION = 3` line 294 |
| D6 (atomic save: v3 + v2 cleanup) | PASS | `save_bundle_record_and_events` lines 503-597 |
| D7 (read: v3 first, v2 second) | PASS with WARNING | Implementation diverges from scoping D7 step 3 wording: no `events_count > 0` guard. Harmless in practice (chokepoint short-circuits for pre-m9-02 blobs). See `findings[].spec-divergence-d7-events-count-guard` |
| D8 (identity check per chunk) | PASS with WARNING | Identity filter implemented at lines 236-242. Test exercises wrong-value-id; does NOT force a true prefix collision. See `findings[].test-quality-collision-not-forced` |
| D9 (naming: keep `encode_chunk_key` for v3) | PASS | Renames + new helpers per design table |

## Architecture Delta
Architecture manifest is **not_applicable** (local store-layer impact; no external boundary; no deployment change). The `bundle_events_or_legacy` chokepoint (m9-02 D5) is preserved verbatim — the v3/v2 branching is absorbed inside `load_counterexample_bundle_events`, invisible to consumers in `chronos-services`, `chronos-cli`, `chronos-mcp`.

## Commands
| Command | Exit | Subject | Evidence |
|---|---:|---|---|
| `cargo fmt --all -- --check` | 0 | `7f86a1c` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | `7f86a1c` | `/tmp/clippy_out.log` |
| `cargo test -p chronos-store --lib --no-fail-fast` | 0 | `7f86a1c` | 53 passed; 0 failed (incl. 13 m9_04 + 13 m9_02 + 11 m8_07 + cas + diff + error + storage) |
| `cargo test -p chronos-services --tests --no-fail-fast` | 0 | `7f86a1c` | 262 lib + 2 integration; 0 failed |
| `cargo test -p chronos-cli --tests --no-fail-fast` | 0 | `7f86a1c` | 22 tests; 0 failed |
| `cargo build --bin chronos-mcp` | 0 | `7f86a1c` | binary at `/var/home/rubentxu/cargo-targets/debug/chronos-mcp` |
| `cargo test -p chronos-sandbox --test counterexample_tools` (CHRONOS_MCP_PATH set) | 0 | `7f86a1c` | 12 passed (ce1..ce12) |
| `cargo test -p chronos-sandbox --test e2e_connectivity` (CHRONOS_MCP_PATH set) | 0 | `7f86a1c` | 1 passed (test_mcp_server_starts_and_responds) |

## Issues

### CRITICAL

**Task completeness — 3 spec-promised integration tests are missing.** Spec §5 "Per-crate integration (chronos-services, chronos-cli)" lists three concrete tests:

- `chronos_services::counterexample::tests::m9_04_save_load_roundtrip_through_services` — calls `ChronosCounterexampleService::save` with N events, then `events_count` + `bundle_events_or_legacy`, asserts the events match.
- `chronos_cli::replay::tests::m9_04_replay_uses_v3_layout` — saves a bundle via the API, calls `run_replay`, asserts the verdict is "violation".
- `chronos_cli::replay::tests::m9_04_replay_v2_bundle_uses_legacy_path` — hand-crafts a v2 bundle, calls `run_replay`, asserts events flow through the legacy branch.

None of these names appear anywhere in the repo (`grep -rln 'm9_04' crates/chronos-services crates/chronos-cli crates/chronos-mcp` returns no matches). `crates/chronos-cli/tests/` directory does not exist at all. Until the chokepoint is pinned under v3 and v2 fallback at the consumer-wrapper layer, the cycle's behavioural-compliance gate is not satisfied for the spec scenarios that specifically require services+cli round-trips.

### WARNING

**Test quality — range-bound test is tautological.** `m9_04_range_scan_covers_all_chunk_indices` (lines 2023-2038) only asserts `start_key < end_key` after building boundary keys. It does not open a redb table, insert chunks for two bundles, call `collect_bundle_chunks_range`, or count iteration. The scoping §5 test `m9_04_range_scan_is_bounded_to_one_bundle` (insert A=4 + B=10 chunks, range(A) returns only A's 4 chunks) was substituted by this weaker lex-order check. The implementation IS bounded (lines 209-247 build `[prefix||0, prefix||0xFFFFFFFF)` correctly), but a buggy implementation that calls `table.iter()` would still pass this test.

**Test quality — collision containment test does not force a true hash-prefix collision.** `m9_04_collision_containment_via_bundle_id` (lines 2042-2098) writes a v3 chunk whose value carries the wrong `bundle_id`, then asserts the identity filter drops it. The design D8 promised a test-only helper `__force_collision_prefix` that bypasses `blake3::hash` and writes chosen prefixes to force a key-prefix collision. The delivered test only exercises the value-identity path, not the prefix-collision path. R2 disclosure's "collision probability is effectively zero" claim is supported by reasoning but not by a test that pins the prefix-collision containment.

**Spec/design divergence — D7 step 3 `events_count > 0` guard missing in implementation.** Scoping D7 step 3 says "v2 fallback (only if v3 was empty AND summary.events_count > 0)". Implementation (`collect_bundle_chunks`, lines 282-292) does NOT guard on `events_count`; it always falls back to the legacy scan when v3 returns empty. The design doc omits the guard. Harmless in practice because `bundle_events_or_legacy` short-circuits to `bundle.events.clone()` when `bundle.events` is non-empty (the only case where a bundle with `events_count == 0` could have chunks is pre-m9-02 blob-embedded). But contractually loose.

### SUGGESTION

**R4 disclosure has no specific pinning test.** R4 claims v2 bundles never re-saved keep using the legacy path on read. `m9_04_v2_bundle_loads_via_fallback` covers the load path but does not pin the persistence invariant. A small "load a never-resaved v2 bundle twice; both reads return same events; both go through fallback" test would pin R4.

**R6 disclosure has no specific pinning test.** `KNOWN_BUNDLE_SCHEMA_VERSIONS = [1, 2, 3]` is dead-code with no test asserting the array contents. A trivial `assert!(KNOWN_BUNDLE_SCHEMA_VERSIONS.contains(&3))` would pin R6.

## Lens Summary
| Lens | Findings | Evidence gaps |
|---|---|---|
| `spec-compliance` (A-min default) | 1 critical, 1 medium, 2 low | the 3 missing integration tests would have closed most spec-compliance gaps; a stronger range-bound test would close one more |
| `test-quality` (A-min default) | 1 medium (range bound), 1 low (R4), 1 low (R6) | forced-prefix-collision test; range-bound test with two bundles; R4/R6 pinning tests |

## Verdict

**FAIL**

Reason tied to mandatory gates:
- `task_completeness`: FAIL — three integration tests listed as required by spec §5 and scoping §7 T2 were not delivered. The chokepoint is preserved in production code but is not pinned by tests under the new v3 layout at the consumer-wrapper layer.
- `test_strength`: FAIL — two of the in-tree tests are weaker than the spec promised (range bound and collision containment).

The store-layer implementation is correct, the lib-level tests are green, and all deterministic gates pass (T0, T1, T2, T4-smoke). The cycle's debt finding (`FIND-M9-02-DV-PERF-01`) is closed by the implementation. But the verification gate requires spec compliance, and three spec-listed scenarios are UNPINNED.

**Recommended next step**: open a follow-up correction cycle (`feat/m9-04-correctness-tests` or similar) that adds the three spec-promised integration tests plus the stronger range-bound + forced-collision tests, then re-runs verify. The current cycle should NOT proceed to release/archive until those tests exist; once they do, re-verify can flip the verdict to PASS.