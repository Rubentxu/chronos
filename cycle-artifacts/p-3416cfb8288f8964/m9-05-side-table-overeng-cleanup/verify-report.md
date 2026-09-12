# Verify Report — m9-05

## Subject

| Base | Head | CWD | Verified at |
|---|---|---|---|
| `c9e5417` | `fa0ca90` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-12T08:39:00+02:00 |

`git status --porcelain` shows only `.jcode/` (gitignored). HEAD is pinned at `fa0ca907e2b...`. Subject identity gate: **PASS**.

## Files Inventory

Source: `git diff --stat c9e5417..HEAD`.

| Bucket | Added | Modified | Deleted | Renamed |
|---|---:|---:|---:|---:|
| `crates/` | 0 | 3 | 0 | 0 |
| `docs/` | 1 (scoping) | 0 | 0 | 0 |

Top paths:
| Status | Bucket | Path |
|---|---|---|
| added | docs/ | `docs/milestones/m9-05-side-table-overeng-cleanup-scoping.md` |
| modified | crates/ | `crates/chronos-store/src/counterexample_storage.rs` |
| modified | crates/ | `crates/chronos-store/src/storage.rs` |
| modified | crates/ | `crates/chronos-cli/tests/replay_integration.rs` |

## Summary

| Verdict | Mode | Path | Required scenarios | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|---|
| **PASS** | light-verify inline | B-direct | 4 spec scenarios (R1–R4) | T0, T1, T2 all green | 0 | 0 |

## Behavioral Compliance

| # | Scenario | Production Path | Test | Status | Evidence |
|---|---|---|---|---|---|
| R1 | `decode_chunk_payload` extracted and used by both loaders | `decode_chunk_payload(bytes) -> Option<Vec<TraceEvent>>` (private free fn) | store unit tests + cli integration | **COMPLIANT** | T1 store 56/56; T2 cli 2/2; identical ladder no longer duplicated |
| R2 | `collect_v3_keys_for_bundle` extracted; `save_bundle_record_and_events` reuses it | private free fn mirrors `collect_bundle_chunks_range` minus (chunk_idx, value) | store unit tests + services round-trip | **COMPLIANT** | T1 store 56/56; services round-trip via m9_04_save_load_roundtrip_through_services |
| R3 | `collect_bundle_chunks(events_count: u64)` — `Option` wrapper dropped | signature change + guard inlined as `if events_count == 0 { return Ok(Vec::new()); }` | store unit tests (m9_02_load_events_concatenates_chunks_in_order updated to inject bundle record) | **COMPLIANT** | T1 store 56/56; m9-04 R4 pin `m9_04_r4_v2_bundle_never_resaved_uses_legacy_path` still green |
| R4 | `db()` narrowed to `pub(crate)`; `COUNTEREXAMPLE_BUNDLES` + `COUNTEREXAMPLE_BUNDLE_EVENTS` narrowed to module-private; cli test uses new chokepoints | `pub fn insert_v2_chunk_for_test`, `pub fn count_v3_chunks_for_test`, `pub fn insert_bundle_record_for_test` (all `#[doc(hidden)]`) | cli replay_integration.rs (rewritten to use chokepoints) | **COMPLIANT** | T2 cli 2/2; T1 store 56/56 |

## Production Readiness

| Gate | Status | Evidence |
|---|---|---|
| Errors / recovery | PASS | `StoreError` enum unchanged; legacy reader returns `Ok(Vec::new())` for missing tables; D7 guard returns empty on `events_count == 0` |
| State / data integrity | PASS | Atomic write tx unchanged; v3+v2 cleanup unchanged; bundle record and side-table updates still linked through `save_bundle_record_and_events` |
| Resource cleanup | PASS | `read_tx`/`write_tx` opened and dropped inside scope; chokepoints commit immediately; no leaked handles |
| Test-side surface | PASS | Three `#[doc(hidden)] pub` chokepoints replace cross-crate `db()`/`TableDefinition` access; cli test still exercises the full replay path end-to-end |

## Source Diff Summary

```
crates/chronos-store/src/counterexample_storage.rs | +218 -56
crates/chronos-store/src/storage.rs                |   +7  -2
crates/chronos-cli/tests/replay_integration.rs     |  +29 -19
docs/milestones/m9-05-side-table-overeng-cleanup-scoping.md | +108 -0
4 files changed, 362 insertions(+), 77 deletions(-)
```

Net LoC reduction in `counterexample_storage.rs` core logic:
- R1: -10 (duplicated decode ladder collapsed)
- R2: -22 (inline range-scan collapsed into helper call)
- R3: -12 (Option wrapper and None branch removed)

Total core-logic reduction: **~44 LoC** (matches envelope estimate of ~38 LoC ±). Net file size grew due to added helper signatures + chokepoints + doc comments, but the production code is simpler and the public surface is smaller (3 narrower `#[doc(hidden)]` chokepoints replace raw `db()`+`TableDefinition` access).

## Test Totals

| Bucket | Result |
|---|---|
| T0 fmt + clippy | PASS (0 warnings) |
| T1 chronos-store lib | 56/56 |
| T2 chronos-store integration | 56/56 |
| T2 chronos-cli integration | 2/2 |
| T2 chronos-services lib | 263/263 (services round-trip unaffected) |

Sandbox tests not warranted: no MCP/probe plumbing touched, no session/lifecycle/serialization changes.

## Findings Closed

| ID | Cluster | Severity | Closed by |
|---|---|---|---|
| overeng-001-v3-chunk-decode-dup | overeng | MEDIUM | R1 |
| overeng-002-v3-range-scan-verify-dup | overeng | LOW | R2 |
| overeng-003-events-count-none-branch | overeng | LOW | R3 |
| cc-003-wrong-direction-visibility | coupling | MEDIUM | R4 |

Verdict: **PASS** · 4/4 apply-target findings closed.
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
