# Implementation Receipt — m9-80-property-policy-ownership

**Commit**: ccf8811 (feat/m9-80-property-policy-ownership, base 82e219f)
**Branch**: feat/m9-80-property-policy-ownership

## Changes

Move four property-policy primitives out of `chronos_services::hypothesis_test`
into `chronos_domain::property`, following the spec rev 2 layered split
(domain owns the policy, services owns the wire shape). The refactor is internal
to the Rust crate graph; no MCP schema, no JSON envelope, no `session_*` action
signature changes.

| File | Diff |
|---|---|
| `crates/chronos-domain/src/property.rs` | +777/-165. Adds 7 domain-owned types (`PropertyHypothesisVerdict`, `PropertyObservationSource`, `InvariantOutcome`, `PropertyExistencePredicate`, `ExistenceOutcome`, `CallPathOutcome`, `PropertyHypothesisOutcome` — all serde-derived) and 4 pub fns (`eval_invariant`, `eval_existence`, `eval_call_path`, `observe_property_target`) plus a private `bfs_reach_domain` BFS helper. Adds module-level doc comments explaining verdict semantics for each kind. |
| `crates/chronos-services/src/hypothesis_test.rs` | -239/+35. Removes the 4 services-layer private fns (`eval_invariant`, `eval_existence`, `eval_call_path`, `observe_property_target`) and 6 helpers (`parse_property_value`, `outcome_to_envelope`, `scan_predicate`, `predicate_label`, `event_type_label`, `bfs_reach`). The 4 match arms in `HypothesisTest::test` now call `chronos_domain::property::eval_*` and wrap the outcome via `HypothesisOutput::from(outcome)`. The Invariant arm preserves the original missing-property_target short-circuit (verbatim verdict reason) before invoking the domain fn. Unused imports (`HashSet`, `VecDeque`, `EventData`, `EventType`, `TraceEvent` in production; `PropertyOutcome` dropped) trimmed; `TraceEvent` re-pinned in the `cfg(test)` inner use. |
| `crates/chronos-services/src/output.rs` | +20. Adds 6 `From` impls: `PropertyHypothesisVerdict → HypothesisVerdict`, `PropertyObservationSource → HypothesisScope`, `PropertyExistencePredicate → ExistencePredicate` (with VariableRead/EventTypeOnly fallback), `InvariantOutcome → HypothesisOutput`, `ExistenceOutcome → HypothesisOutput`, `CallPathOutcome → HypothesisOutput`. |
| `crates/chronos-domain/src/lib.rs` | extends the `pub use property::{…}` block (line 28) with `Property`, `PropertyHypothesisOutcome`, `InvariantOutcome`, `ExistenceOutcome`, `CallPathOutcome`, `PropertyHypothesisVerdict`, `PropertyObservationSource`, `PropertyExistencePredicate`. `Property` was already re-exported since T0; the new types are added in T0. |

No public API surface change: `HypothesisTest::test` retains its signature
`pub async fn test(ctx, input) -> Result<HypothesisOutput, ServiceError>`.
The wire shape (`HypothesisOutput::{Invariant, Existence, CallPath}` variants)
is preserved through the From impls.

## Commit timeline (since base)

| SHA | Title | Phase |
|---|---|---|
| `90c7d4c` | m9-80: define PropertyHypothesisOutcome family in domain (T0) | T0 |
| `ae6a7df` | m9-80: add T0 progress note (domain outcome types landed) | T0 doc |
| `a12e7d6` | m9-80: move T0 progress note from cycle-artifacts/ to changes/ | T0 vault (CC#18) |
| `d1e6a52` | vault: add m9-80 row + CC#51 exception + CC#6 last-closed fix + CC#4 cascade | T0 vault |
| `57c1a25` | docs(handoff): append m9-80 T0 complete + 3 vault fixes (CC#5/6/51) + T1 next-session procedure | T0 handoff |
| `7990db9` | m9-80: move eval_invariant into chronos_domain::property (T1) | T1 |
| `c6c7efe` | docs(handoff): append m9-80 T1 complete + repeatable pattern for T2-T5 | T1 handoff |
| `933670d` | m9-80: move eval_existence into chronos_domain::property (T2) | T2 |
| `26a5cf4` | feat(m9-80): move eval_call_path from services to domain (T3) | T3 |
| `3d93766` | m9-80: rename observe_property_target_domain + re-export Property (T4) | T4 |
| `ccf8811` | m9-80: make observe_property_target public + final verification (T5) | T5 |

10 commits since base; 4 code files (`crates/chronos-domain/src/lib.rs`,
`crates/chronos-domain/src/property.rs`, `crates/chronos-services/src/hypothesis_test.rs`,
`crates/chronos-services/src/output.rs`) carry the substantive changes. The other
14 file changes are documentation, handoff appends, vault index maintenance, and
the 10 archive-manifest.md CC#4 cascades.

## Gates (orchestrator-verified 2026-09-14T08:40Z)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (0 violations) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0 warnings, 0 errors) |
| `cargo build --workspace` | PASS (7 s incremental after dirty domain; 20 s cold) |
| `cargo test -p chronos-services -p chronos-domain --lib` | PASS (264 + 149 = 413/413) |
| `cargo test -p chronos-services --lib hypothesis_test` | PASS (13/13; 67.31 s) |
| `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e --exclude chronos-native --no-fail-fast` | PASS (44 lib suites, 0 failures) |
| `cargo test -p chronos-native --lib -- --test-threads=1` | PASS (103/103; 13.03 s) |
| T4-smoke `chronos-sandbox --test session_lifecycle --test analytics_tools --test program_scenarios` (`--test-threads=1`) | PASS (4 + 11 + 8 = 23/23; 167 s wall) |
| `python3 scripts/regen_manifest_index_shas.py` | no-op (77 manifests already correct) |
| `bash scripts/check_vault_drift.sh` | PASS (48 python CCs + 7 bash CCs) |

## Acceptance mapping

The cycle has 3 spec requirements × 3 scenarios each = 9 mechanical scenarios.
All 6 grep-based mechanical scenarios pass (see verify-report.md for the table).
All 3 unit-test scenarios pass (264 services + 149 domain + 13 hypothesis_test +
103 native + 23 sandbox = 552 tests across all tiers).

The 13 hypothesis_test unit tests cover:
- 3 invariant tests: `invariant_event_count_ge_violation`, `invariant_event_count_lt_pass`,
  `invariant_latency_ms_unsupported_with_reason`
- 3 property-value invariant tests: `invariant_property_value_eq_pass`,
  `invariant_property_value_missing_unsupported`
- 3 existence tests: `existence_empty_session_unsupported`, `existence_event_type_pass`,
  `existence_no_match_violation`
- 4 call-path tests: `call_path_missing_caller_unsupported`, `call_path_self_loop_pass`,
  `call_path_unreachable_violation`, `call_path_reachable_pass`
- 1 coordinator test: `session_not_found_returns_error`

All assertions unchanged from base (test bodies may have gained `use` imports to
reach the domain API, but no assertion string/int was modified).

## Deviations

The original spec (rev 1) said "byte-for-byte move"; recon at T1 startup
showed the four functions reference services-layer output types
(`HypothesisOutput`, `HypothesisKind`, `HypothesisScope`, `HypothesisVerdict`,
`ExistencePredicate`). A literal byte-for-byte move would force
`chronos-domain → chronos-services` (circular). The spec was revised at base
(`82e219f`) to a layered split: domain returns `PropertyHypothesisOutcome`-family
types; services wraps via `From`. The implementation followed rev 2.

A micro-deviation in T5: `observe_property_target` was promoted from `fn` (private)
to `pub fn` to satisfy the spec's literal grep scenario
`domain_property_has_four_pub_functions` (which expects 4 `pub fn` matches).
The promotion has zero functional impact (no caller in services uses it
directly; it's still consumed internally by `eval_invariant`) and unblocks
future MCP callers that want to invoke observation without going through the
invariant wrapper.

## Carry-forward findings (unchanged from base)

- FIND-M9-75, FIND-M9-74, FIND-M9-72, FIND-M9-71: unchanged.
- FIND-M9-66 (broken JSON in m9-66 verify-findings.json): unchanged.
- Pre-existing smoke-test work-copy isolation flake: unchanged;
  `scripts/smoke_test_ccs.sh` not used as a CI gate.
