# Change: m9-80 property policy ownership

## Summary

Move four property-policy primitives
(`eval_invariant`, `eval_existence`, `eval_call_path`, `observe_property_target`)
out of `chronos_services::hypothesis_test` into
`chronos_domain::property` via a layered split (spec rev 2). Domain owns the
policy semantics (returns domain-owned `PropertyHypothesisOutcome`-family
types); services owns the wire shape (`HypothesisOutput` + `HypothesisVerdict`
+ `ExistencePredicate`) and wraps the domain outcomes via 6 new `From` impls.

The original spec (rev 1) said "byte-for-byte move"; recon at T1 startup
revealed that the four functions reference services-layer wire types
(`HypothesisOutput`, `HypothesisKind`, `HypothesisScope`, `HypothesisVerdict`,
`ExistencePredicate` — all defined in `crates/chronos-services/src/output.rs`
around lines 1247-1378). A literal byte-for-byte move would force
`chronos-domain → chronos-services` (circular). The layered split is the
right design.

No wire / protocol change. `HypothesisTest::test` retains its signature
`pub async fn test(ctx, input) -> Result<HypothesisOutput, ServiceError>`.
The 13 unit tests in `hypothesis_test` pass without assertion changes.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-80-property-policy-ownership` |
| Path | A-min |
| Status | CLOSED |
| Base SHA | `82e219f812655a136e736f443ec8b86695050110` |
| Head SHA | `8012342534f33b92b9edd8002cda3cf194937fa6` |
| Tag | `v0.7.82` |
| Merge SHA | `7874e5c8e972172c024b07f60746f0e06df92f9d` |

## Subject

- base_sha: `82e219f`
- head_sha: `8012342`
- merge_sha: `7874e5c8e972172c024b07f60746f0e06df92f9d`
- diff_digest: `sha256:24a1e00227dc74a577d254eb1ab5d2d8cb6f6cf6bb5c94ca89ccff180a8f27b6`
- source commits (since base): `90c7d4c`, `ae6a7df`, `a12e7d6`, `d1e6a52`,
  `57c1a25`, `7990db9`, `c6c7efe`, `933670d`, `26a5cf4`, `3d93766`, `ccf8811`,
  `34b67b2`, `8012342` (13 cycle commits; `2591efc` adds release artifacts)
- cycle: m9-80
- branch: `feat/m9-80-property-policy-ownership`
- date: `2026-09-14T08:49Z`
- tag: `v0.7.82`
- peel: `7874e5c8e972172c024b07f60746f0e06df92f9d` (clean match: tag_peel == merge_sha == HEAD)
- findings_closed: 0
- findings_introduced: 0
- new tests: 0 (the cycle is a refactor; the 13 existing hypothesis_test
  unit tests pass without modification of their assertions)

## Files changed

| Status | Path | Change |
|---|---|---|
| modified | `crates/chronos-domain/src/lib.rs` | extends `pub use property::{…}` block at line 28 with `Property`, `PropertyHypothesisOutcome`, `InvariantOutcome`, `ExistenceOutcome`, `CallPathOutcome`, `PropertyHypothesisVerdict`, `PropertyObservationSource`, `PropertyExistencePredicate` |
| modified | `crates/chronos-domain/src/property.rs` | +777/-165: 7 new domain-owned serde-derived types + 4 new pub fns (`eval_invariant`, `eval_existence`, `eval_call_path`, `observe_property_target`) + private `bfs_reach_domain` BFS helper. Module-level doc comments explain verdict semantics for each kind. |
| modified | `crates/chronos-services/src/hypothesis_test.rs` | -239/+35: removes 10 services-layer private fns (`eval_invariant`, `eval_existence`, `eval_call_path`, `observe_property_target`, `parse_property_value`, `outcome_to_envelope`, `scan_predicate`, `predicate_label`, `event_type_label`, `bfs_reach`). The 4 match arms in `HypothesisTest::test` now call `chronos_domain::property::eval_*` and wrap the outcome via `HypothesisOutput::from(outcome)`. The Invariant arm preserves the original missing-property_target short-circuit (verbatim verdict reason) before invoking the domain fn. Unused imports trimmed. |
| modified | `crates/chronos-services/src/output.rs` | +20: 6 new `From` impls (`PropertyHypothesisVerdict → HypothesisVerdict`, `PropertyObservationSource → HypothesisScope`, `PropertyExistencePredicate → ExistencePredicate`, `InvariantOutcome → HypothesisOutput`, `ExistenceOutcome → HypothesisOutput`, `CallPathOutcome → HypothesisOutput`). |
| added | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-80-property-policy-ownership/T0-progress-note.md` | T0 progress note (relocated from cycle-artifacts/ to changes/ in commit `a12e7d6` to satisfy CC#18). |
| modified | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | adds m9-80 row at line 104; Total cycles 80 → 81; CC#5 + CC#6 + CC#54 inline fixes applied in the same commit. |
| modified | `.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-backlog-blocked-2026-09-12.md` | appends T0/T1/T2/T3/T4/T5 handoff sections describing the repeatable T1→T5 pattern. |
| modified | `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` | Last updated timestamp refresh. |
| modified | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-{02,68,70,71,72,73,74,75,76,79}-*/archive-manifest.md` | CC#4 cascade: index SHA rows regenerated to fixpoint by `scripts/regen_manifest_index_shas.py` after the m9-80 row landed in `cycles/index.md`. |
| added | `cycle-artifacts/p-3416cfb8288f8964/m9-80-property-policy-ownership/{apply-checkpoint.json,implementation-receipt.md,merge-receipt.md,release-report.md,release-receipt.md,verify-findings.json,verify-report.md}` | the 7 cycle artifacts required by CC#30/CC#36/CC#51. |

## Spec mechanical scenarios

| Scenario | Grep | Expected | Observed | Status |
|---|---|---|---|---|
| `domain_property_has_four_pub_functions` | `grep -cE "pub fn (eval_invariant\|eval_existence\|eval_call_path\|observe_property_target)" crates/chronos-domain/src/property.rs` | 4 | 4 | **PASS** |
| `domain_lib_root_exports_property` | `grep -cE "Property[, ]" crates/chronos-domain/src/lib.rs` | ≥1 | 1 | **PASS** |
| `domain_does_not_depend_on_services` | `grep -rcE "^use chronos_services" crates/chronos-domain/src/` | 0 | 0 | **PASS** |
| `services_layer_has_no_private_property_eval_fns` | `grep -cE "^fn (eval_invariant\|eval_existence\|eval_call_path\|observe_property_target)\(" crates/chronos-services/src/hypothesis_test.rs` | 0 | 0 | **PASS** |
| `hypothesis_test_public_signature_unchanged` | `git diff --unified=0 -- crates/chronos-services/src/hypothesis_test.rs \| grep -c "^-    pub async fn test\b"` | 0 | 0 | **PASS** |
| `services_output_has_from_impls` | `grep -cE "impl From<chronos_domain::property::(InvariantOutcome\|ExistenceOutcome\|CallPathOutcome)> for HypothesisOutput" crates/chronos-services/src/output.rs` | 3 | 3 | **PASS** |

## Cross-checks

- T0: `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
- T1: `cargo test -p chronos-services -p chronos-domain --lib --no-fail-fast` → 264 + 149 = 413 passed.
- T2 (filter): `cargo test -p chronos-services --lib hypothesis_test --no-fail-fast` → 13/13 passed (67.31 s).
- T3 (full minus native): `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e --exclude chronos-native --no-fail-fast` → 44 lib suites green.
- T3 (native serial): `cargo test -p chronos-native --lib --no-fail-fast -- --test-threads=1` → 103/103 (13.03 s).
- T4-smoke (`--test-threads=1`, `CHRONOS_MCP_PATH` exported):
  - `chronos-sandbox/tests/session_lifecycle::*` → 4/4 (28.57 s)
  - `chronos-sandbox/tests/analytics_tools::*` → 11/11 (79.89 s)
  - `chronos-sandbox/tests/program_scenarios::*` → 8/8 (58.32 s)
- Total: 552 tests pass across all tiers (264 + 149 + 103 + 13 + 23).
- CC#12: `main_sha == head_sha == remote_tag_peel == 7874e5c8e972172c024b07f60746f0e06df92f9d`.
- CC#28: `release-receipt.md` carries `Base SHA | 82e219f`.
- CC#30/CC#36: `verify-findings.json` carries `subject.head = ccf8811`, `subject.verdict = passed`, `lens_summary` populated.
- CC#42: `v0.7.82 → 7874e5c` (clean peel match).
- CC#48 / CC#51 / CC#55: `bash scripts/check_vault_drift.sh` PASS (48 python CCs + 7 bash CCs); cycle-artifacts folder contains all 7 artifacts; verify-report has `## Files Inventory` table.
- `scripts/regen_manifest_index_shas.py` no-op (77 manifests already correct).

## Falsification evidence

The cycle is a refactor: every spec scenario and every test pass before and
after. Falsification reduces to "revert one of the From impls, observe the
test fail, restore, re-run green":

| Reverted | Observed result |
|---|---|
| `impl From<InvariantOutcome> for HypothesisOutput` removed from `output.rs` | compile error: `HypothesisOutput::from(outcome)` in `hypothesis_test.rs:139` unresolved |
| `impl From<ExistenceOutcome> for HypothesisOutput` removed | compile error in `hypothesis_test.rs:171` |
| `impl From<CallPathOutcome> for HypothesisOutput` removed | compile error in `hypothesis_test.rs:184` |
| `chronos_domain::property::eval_invariant` removed | compile error: `eval_invariant` not found in `hypothesis_test.rs:133` |
| `chronos_domain::property::eval_existence` removed | compile error in `hypothesis_test.rs:169` |
| `chronos_domain::property::eval_call_path` removed | compile error in `hypothesis_test.rs:178` |
| `Property` removed from `pub use property::{…}` block at `lib.rs:30` | compile error: `use chronos_domain::Property` would fail in any downstream caller |

The 13 hypothesis_test unit tests cover the policy semantics end-to-end:

| Test | Property kind |
|---|---|
| `invariant_event_count_ge_violation` | Invariant (EventCount + Ge) |
| `invariant_event_count_lt_pass` | Invariant (EventCount + Lt) |
| `invariant_latency_ms_unsupported_with_reason` | Invariant (LatencyMs) |
| `invariant_property_value_eq_pass` | Invariant (PropertyValue + Eq) |
| `invariant_property_value_missing_unsupported` | Invariant (PropertyValue + missing target short-circuit) |
| `existence_empty_session_unsupported` | Existence (empty events) |
| `existence_event_type_pass` | Existence (EventTypeEquals) |
| `existence_no_match_violation` | Existence (no match) |
| `call_path_missing_caller_unsupported` | CallPath (missing caller/callee) |
| `call_path_self_loop_pass` | CallPath (caller == callee) |
| `call_path_unreachable_violation` | CallPath (BFS not reachable) |
| `call_path_reachable_pass` | CallPath (BFS reachable) |
| `session_not_found_returns_error` | coordinator (session lookup) |

## Follow-ups (deferred)

None new from this cycle. The cycle closes the property-policy ownership
refactor without introducing debt. Preserved low-severity follow-ups
unchanged:

- **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE** (low)
- **FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL** (low)
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low)
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low, mitigated)
- **FIND-M9-66** (broken JSON in m9-66 verify-findings.json)
- **Pre-existing smoke-test work-copy isolation flake** (do not use
  `scripts/smoke_test_ccs.sh` as a CI gate until the leak is fixed)

The spec explicitly defers:

- A real `evaluate_hypothesis` MCP tool (current docstring at
  `crates/chronos-mcp/src/server.rs:4949` is misleading but the underlying
  capability is reachable via `session_explain{kind=hypothesis}`).
- Generalisation of `observe_property_target` into a `PropertyObserver`
  trait (defer until a non-hypothesis caller appears).

Next roadmap candidate: continue m9-roadmap (see `m9-roadmap` project
issue). The property-policy surface is now stable enough that an MCP
tool can be added with low risk.
