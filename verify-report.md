# Verification Report: m9-01-schema-versioning

## Subject

| Base | Head (final) | Dirty diff digest | CWD | Verified at |
|---|---|---|---|---|
| `bceddc946b73209756d3360b0aec4dca592296c3` | `0aec8bac30f995e638e2a656b55935f48a60623d` | `e60cd27a9aa285730968c0705b92069dbdde0eee2fc2c1b81afd1aa392027bd9` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-11T17:30:00Z |

Head was `cb4de8d` (3 commits: scoping + feat + apply-checkpoint) at verify start. During re-execution of T0, fmt drift was detected in the test bodies added by `c346d8b`. The cycle was completed without blocking by committing a `chore(fmt)` correction (commit `0aec8ba`); cycle remains single-concept (one semantic change: schema_version). Working tree is clean at verify end.

Cycle commits on top of base:

| SHA | Subject |
|---|---|
| `0060a27` | docs(m9-01): scoping — schema_version on CounterexampleBundleRecord/Summary |
| `c346d8b` | feat(chronos-store): schema_version on CounterexampleBundleRecord/Summary (m9-01) |
| `cb4de8d` | chore(m9-01): apply-checkpoint after T4-smoke pass |
| `0aec8ba` | chore(fmt): apply rustfmt to m9-01 new test bodies (verify correction) |

## Files Inventory

Cycle touches 3 production files (chronos-store, chronos-services, chronos-cli test fixtures only) + 1 milestone doc.

| Bucket | Added | Modified | Deleted | Renamed |
|---|---:|---:|---:|---:|
| crates/ | 0 | 3 | 0 | 0 |
| docs/milestones/ | 1 | 0 | 0 | 0 |

Top paths by status / bucket:

| Status | Bucket | Path | Renamed from | SHA-256 |
|---|---|---|---|---|
| modified | crates/ | `crates/chronos-store/src/counterexample_storage.rs` | — | (production + 5 new tests; +307 lines) |
| modified | crates/ | `crates/chronos-services/src/counterexample.rs` | — | (production: 2 lines; +1 test) |
| modified | crates/ | `crates/chronos-cli/src/replay.rs` | — | (test fixtures only: 6 lines) |
| added | docs/milestones/ | `docs/milestones/m9-01-schema-versioning-scoping.md` | — | (518 lines) |

The `inventory.json` SDDK CLI artifact was not generated because the cycle has no runtime record in `sddk cycle status` (sddk CLI returns `STORAGE_NOT_FOUND: cycle not found: m9-01-schema-versioning`). This is an ad-hoc cycle, not registered with the SDDK ledger; the orchestrator owns the runtime transition path.

## Summary

| Verdict | Mode | Path | Required scenarios | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|---|
| **PASS_WITH_WARNINGS** | normal | A-min | 8 (G1–G8) + 4 disclosures (R1–R4) | 6/6 | 0 | 1 |

## Behavioral Compliance

| # | Requirement / Scenario | Production Path | Test | Status | Evidence |
|---|---|---|---|---|---|
| G1 | `schema_version: u32` on `CounterexampleBundleSummary` | `crates/chronos-store/src/counterexample_storage.rs:172-178` | `m9_01_save_writes_schema_version_1` | COMPLIANT | T1 OK, `loaded.summary.schema_version == 1` |
| G2 | `schema_version: u32` on `CounterexampleBundleRecord` (envelope mirror) | `crates/chronos-store/src/counterexample_storage.rs:200-206` | `m9_01_save_writes_schema_version_1` | COMPLIANT | T1 OK, `loaded.schema_version == 1` |
| G3 | `#[serde(default = "default_schema_version")]` on both fields; legacy bundles deserialize as 1 | `counterexample_storage.rs:63-65, 177, 205` | `m9_01_legacy_bundle_deserializes_with_schema_version_1` | COMPLIANT | T1 OK, JSON without field deserializes to `schema_version: 1` |
| G4 | `pub const CURRENT_BUNDLE_SCHEMA_VERSION: u32 = 1` + `KNOWN_BUNDLE_SCHEMA_VERSIONS` | `counterexample_storage.rs:51, 55-58` | (compile + grep) | COMPLIANT | constant visible to services via `cs::CURRENT_BUNDLE_SCHEMA_VERSION`; services `save()` references it at `counterexample.rs:489, 497` |
| G5 | Loader rejects `schema_version > CURRENT` with actionable message | `counterexample_storage.rs:289-298` | `m9_01_future_version_load_is_rejected` + `m9_01_list_includes_future_versioned_row_best_effort` (negative load assertion) | COMPLIANT | T1 OK; error message `"bundle schema_version 2 is newer than supported 1; upgrade chronos-store to read this bundle"` |
| G6 | `save_counterexample_bundle` overwrites both fields with `CURRENT` | `counterexample_storage.rs:232-235` | `m9_01_save_overwrites_callers_schema_version` | COMPLIANT | T1 OK, caller sets 99 → persisted value 1 |
| G7 | chronos-services save passes the constant through the wire summary | `crates/chronos-services/src/counterexample.rs:489, 497` | `m9_01_save_persists_schema_version_1` | COMPLIANT | T2 services OK, end-to-end save+load returns schema_version 1 |
| G8 | All three contract tests + overwrites + list-best-effort pass | (T1 + T2) | 5 unit + 1 integration | COMPLIANT | T1 32/32, T2 services 260/260, store integration 32/32 |

| Disclosure | Validation | Evidence |
|---|---|---|
| R1 (schema_version silently overwritten on save, D5 canonical writer) | Confirmed in code + test | `counterexample_storage.rs:232-235` + `m9_01_save_overwrites_callers_schema_version` |
| R2 (future-versioned bundles appear in `list`, rejected individually on `load`) | Confirmed in code + test | `counterexample_storage.rs:289-298` (load-only reject) + `m9_01_list_includes_future_versioned_row_best_effort` (2-row list + per-row reject) |
| R3 (internal-only, not on MCP wire or services-side summary) | Confirmed: services-side `CounterexampleBundleSummary` (line 247) has 6 fields, no `schema_version`; MCP DTO unchanged | `grep -n schema_version crates/chronos-services crates/chronos-mcp/src/output.rs` returns no DTO field |
| R4 (`KNOWN_BUNDLE_SCHEMA_VERSIONS` currently unused) | Confirmed: declared at `counterexample_storage.rs:55-58` with `#[allow(dead_code)]`; loader checks only upper bound (line 293) | grep + read |

## Production Readiness

| Gate | Status | Evidence | Findings / N/A reason |
|---|---|---|---|
| Subject identity (clean commit tree) | PASS | `git status --porcelain` empty at verify end; base = `bceddc9`, head = `0aec8ba` | none |
| Behavioral compliance | PASS | All 8 §2 goals + 4 disclosures mapped to a passing test | none |
| Real implementation (no stub / hardcode / fake) | PASS | All production changes are wire-format constants + serde attributes + a 5-line if-reject; no TODO / FIXME / `todo!` / `unimplemented!` in changed paths (`grep -E "TODO|FIXME|HACK|todo!\|unimplemented!\|NotImplemented"` on diff returns 0 hits) | none |
| Documentation discipline (no issue / PR / task / user refs in comments) | PASS | Diff contains only `m9-01` milestone identifiers attached to behaviour ("m9-01: monotonic version..."), which are scoping IDs, not traceability-only pointers | none |
| Test strength | PASS | All 5 unit tests are positive + negative controls; `m9_01_future_version_load_is_rejected` is the negative control (hand-crafted v2 blob → expected `Err` with specific substring); `m9_01_save_overwrites_callers_schema_version` is a control for D5; no tautologies, no snapshot-only assertions, no mock-only proofs (real `SessionStore::in_memory` + real bincode + real redb write in tests) | none |
| Regression and build (T0/T1/T2) | PASS_WITH_WARNING | T0 (fmt + clippy) PASS after one `chore(fmt)` correction commit; T1 (32/32), T2 services (260/260), T2 store-integration (32/32) all pass | 1 WARNING: fmt drift caught during verify, fixed in 0aec8ba |
| Production readiness matrix | PASS | Errors/recovery: rejected `Err(StoreError::Serialization)` is wrapped by the existing ServiceError path (replay.rs); state/data integrity: save always writes CURRENT (no caller-trusted version); resource cleanup: unchanged; concurrency: unchanged; migrations: see R2 disclosure (best-effort skip in list); security: N/A (no input boundary change); performance: N/A (single integer compare per load); observability/deployability: error message names the version and the fix | none |
| Design and SOLID | PASS | SRP: counterexample_storage.rs owns one envelope; OCP: future version bumps touch only the constant + loader branch; LSP: serde default machinery preserves round-trip for v1; ISP: services-side summary unchanged (D4/R3); DIP: services-side save depends on the constant (public re-export), not on internals | none |
| Task completeness | PASS | Scoping listed 5 unit + 1 integration test; all present and green; no deferred optional work | none |
| Apply-push discipline | N/A | No `apply-report.md` artifact in cycle; no remote `git push` invoked; ad-hoc cycle not registered with sddk runtime (`sddk cycle status` returns STORAGE_NOT_FOUND) | see "Runtime context" below |
| Pre-commit discipline (subject clean) | PASS | `git status --porcelain` empty | none |

## Code Quality

| Standard | Status | Evidence | Findings |
|---|---|---|---|
| Business code reality (no stub / mock / hardcoded satisfier) | PASS | `git diff | grep -E "TODO|FIXME|HACK\|todo!\|unimplemented!\|NotImplemented"` returns 0 hits across all 3 changed files. Production changes are constants + serde attrs + 5-line reject branch. The hardcoded `1` in save() and test fixtures is the canonical version, not a hidden constant. | none |
| Documentation discipline | PASS | Comments in changed production paths: (a) module-level doc paragraph on lines 22-28 explains the *what* (schema versioning) and *why* (legacy default + future-reject); (b) field-level doc comments on lines 173-176, 200-204 explain the semantics + interaction with the loader; (c) test comments include `m9-01 §5` references — these name the milestone that introduced the test, not a tracker ID. No PR / issue / user handles in any comment. | none |

## SOLID And Design

| Principle / Decision | Status | Concrete evidence | Impact |
|---|---|---|---|
| SRP — module owns the bundle envelope | PASS | `counterexample_storage.rs` is the only file that knows the on-disk shape; services-side depends on the public constant. | none |
| OCP — extend by bumping the constant | PASS | Loader branch at line 293 is the only site that needs updating on a v1→v2 bump (plus the constant). No stable module needs patching. | matches D2/D5 |
| LSP — wire contract preserved for v1 | PASS | `#[serde(default)]` machinery means a pre-m9-01 blob deserializes as `schema_version: 1`; downstream code (`reconstruct_hypothesis`, `counterexample_summary_from_wire`) sees the same field set as before. | matches §2 G3 |
| ISP — services-side summary unchanged | PASS | `chronos_services::counterexample::CounterexampleBundleSummary` (line 247) has 6 fields; MCP DTO unchanged. R3 disclosure is honest. | matches D4 |
| DIP — services depends on the public constant | PASS | `cs::CURRENT_BUNDLE_SCHEMA_VERSION` is `pub const` in chronos-store; services `save()` consumes the constant, not the literal `1`. Single source of truth. | matches §4 sketch |

## Architecture Delta

No architecture manifest was declared for this cycle (A-min, no design artifact). The change is internal-schema only (R3 disclosure); MCP surface, LLM wire, and CLI surface are unchanged. Architecture delta is empty.

## Commands

| Command | Exit | Subject | Evidence |
|---|---|---|---|
| `cargo fmt --all -- --check` (pre-fix) | 1 | cb4de8d | drift at lines 839, 945, 964 of counterexample_storage.rs |
| `cargo fmt --all` + `git commit` | 0 | 0aec8ba | chore(fmt) commit, 8 insertions / 3 deletions, cosmetic only |
| `cargo fmt --all -- --check` (post-fix) | 0 | 0aec8ba | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0aec8ba | clean (Fresh profile, no warnings) |
| `cargo test -p chronos-store --lib --no-fail-fast` | 0 | 0aec8ba | 32 passed; 0 failed; includes all 5 m9_01_* tests |
| `cargo test -p chronos-services --lib --no-fail-fast` | 0 | 0aec8ba | 260 passed; 0 failed; includes `m9_01_save_persists_schema_version_1` |
| `cargo test -p chronos-store --tests --no-fail-fast` | 0 | 0aec8ba | 32 passed; 0 failed (integration bucket) |
| `cargo build --bin chronos-mcp` | 0 | 0aec8ba | binary at `target/debug/chronos-mcp`, 137799376 bytes |
| `cargo test -p chronos-sandbox --test e2e_connectivity --test counterexample_tools -- --test-threads=1` | 0 | 0aec8ba | 12 ce1..ce12 + 1 connectivity; total ~72s |

CWD for all commands: `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` with `CARGO_TARGET_DIR=$PWD/target` and `CHRONOS_MCP_PATH=$PWD/target/debug/chronos-mcp` (sandbox subset only).

## Issues

### CRITICAL
(none)

### WARNING

| ID | Title | Evidence | Recommendation |
|---|---|---|---|
| W1 | fmt drift introduced by apply phase (committed by c346d8b without `cargo fmt` post-step) | T0 pre-fix exit 1 at counterexample_storage.rs:839, 945, 964; fixed by chore(fmt) commit 0aec8ba during verify | apply phase should run `cargo fmt --all` after writing new tests and before commit. Cycle outcome unaffected; signals a discipline gap for future cycles. |

### SUGGESTION
(none)

## Lens Summary

This cycle was verified as a single-shot re-execution; no separate `sddk-verify` lenses were dispatched. The A-min path nominally calls for `spec-compliance` + `test-quality` lenses; in this re-run both are covered inline by:

- **spec-compliance** — the Behavioral Compliance table maps every §2 goal (G1–G8) to a passing test exercising production code, and every R1–R4 disclosure to a code location or test assertion.
- **test-quality** — the Production Readiness matrix (`Test strength` row) enumerates negative controls and notes the absence of tautologies, mock-only proofs, and snapshot-only assertions.

| Lens | Findings | Evidence gaps |
|---|---|---|
| spec-compliance (inline) | 0 blocking | none |
| test-quality (inline) | 0 blocking | none |

## Verdict

**PASS_WITH_WARNINGS**

Reason: All mandatory gates pass with fresh evidence (T0 fixed during verify, T1 32/32, T2 services 260/260, T2 store-integration 32/32, T4-smoke counterexample_tools 12/12 + e2e_connectivity 1/1). All 8 §2 goals validated. All 4 R1–R4 disclosures validated in code. The single warning is a fmt drift introduced by the apply phase and corrected in a dedicated commit (0aec8ba); no behavioural regression, no test mutation, no semantic change.

Next: sddk-debt-verify (the path's mandatory successor; A-min cycles always run debt-verify after verify per the orchestrator's lifecycle).

## Runtime context

`sddk cycle status --root . --scope . --cycle m9-01-schema-versioning` returns `STORAGE_NOT_FOUND: cycle not found: m9-01-schema-versioning`. This cycle is not registered with the SDDK ledger (no `sddk-init` / `sddk-plan` / `sddk-propose` steps were run). The verify coordinator therefore did not invoke any lifecycle CLI (no `cycle inventory`, no `cycle evaluate-gate`, no `cycle transition`, no `ledger verify`); the orchestrator that owns the runtime side will translate this PASS_WITH_WARNINGS into its own transition handling when it adopts the cycle. The A-min runtime receipt-ordering blocker (per `prompts/sddk/phases/verify.md` §Ledger Contract) is orthogonal to this product verdict — it applies to a hypothetical transition attempt, which we did not make.

Artifacts persisted:
- `verify-findings.json` (1 warning, 0 critical/high)
- `verify-report.md` (this file)