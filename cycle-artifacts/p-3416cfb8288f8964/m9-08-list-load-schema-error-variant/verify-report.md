# Verify Report — m9-08

## Subject

| Base | Head | CWD | Verified at |
|---|---|---|---|
| `5055396` | `d89862b` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-12T07:02:00+02:00 |

`git status --porcelain` after the fix commit is empty (only `.jcode/` ignored). HEAD is pinned at `d89862b`. Subject identity gate: **PASS**.

## Files Inventory

Source: `git diff --stat 5055396..d89862b`.

| Bucket | Added | Modified | Deleted | Renamed |
|---|---:|---:|---:|---:|
| `crates/` | 0 | 2 | 0 | 0 |

| Status | Bucket | Path |
|---|---|---|
| modified | crates/ | `crates/chronos-store/src/error.rs` |
| modified | crates/ | `crates/chronos-store/src/counterexample_storage.rs` |

## Summary

| Verdict | Mode | Path | Required scenarios | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|---|
| **PASS** | light-verify inline | B-direct | 1 spec scenario (dedicated `SchemaTooNew` variant carries `{ found, supported }`; loader returns it; pattern-match works) | T0, T1, T2 all green | 0 | 0 |

## Behavioral Compliance

| # | Scenario | Production Path | Test | Status | Evidence |
|---|---|---|---|---|---|
| R1 | `StoreError::SchemaTooNew { found, supported }` exists as a dedicated variant and the loader returns it instead of overloading `Serialization` | added variant in `error.rs` with `#[error(...)]` Display + 2-field struct; replaced the loader's `StoreError::Serialization(format!(...))` with `StoreError::SchemaTooNew { found, supported }` in `counterexample_storage.rs` | `error::tests::test_schema_too_new_variant_display_and_fields` (Display + field pattern match); updated `counterexample_storage::tests::m9_02_future_version_load_is_rejected` and `m9_04_saved_v3_schema_and_future_rejection` to pattern-match on the new variant | **COMPLIANT** | T1 60/60 (m9-08 test passes); pre-existing tests still cover the `"newer than supported"` substring (now produced by `SchemaTooNew`'s Display impl) |

The fix is the textbook B-direct remediation prescribed by the debt-report finding itself:

> **Remediation (backlog):** at the first real v2 bump, add a dedicated error
> variant (e.g. `SchemaTooNew { found, supported }`).

The m9-01 cycle shipped three schema versions (`KNOWN_BUNDLE_SCHEMA_VERSIONS = [1, 2, 3]`, `CURRENT_BUNDLE_SCHEMA_VERSION = 3`), so the trigger condition has been satisfied. The previous m9-06 verify-report incorrectly classified this finding as "requires list-side change"; the debt-report itself prescribes only the variant addition. The list-side best-effort semantics (m9-02 R2 disclosure) are preserved unchanged — the asymmetry between list (best-effort) and load (hard-reject) is a deliberate product call, not the target of this finding.

## Production Readiness

| Gate | Status | Evidence |
|---|---|---|
| Errors / recovery | PASS | New variant exposes structured `{ found, supported }` fields; callers no longer need to parse the error message to distinguish "corrupt blob" (`Serialization`) from "forward-compat" (`SchemaTooNew`) |
| State / data integrity | PASS | All legitimate writes still canonicalize to `CURRENT_BUNDLE_SCHEMA_VERSION`; no legitimate path now trips the new variant |
| Backward compatibility | PASS | The `Display` impl emits a string with the same `"newer than supported"` substring the pre-fix `Serialization` message contained, so downstream log scrapers keyed on that phrase still match |
| Resource cleanup | PASS | Pure synchronous error construction; no allocation beyond the variant fields (which are `Copy`-able `u32`) |
| Type safety | PASS | Pattern-matchers on `StoreError::Serialization(_)` no longer accidentally catch forward-compat rejections |

## Source Diff Summary

```
crates/chronos-store/src/error.rs                  | +35
crates/chronos-store/src/counterexample_storage.rs | +32 -5
2 files changed, 67 insertions(+), 5 deletions(-)
```

Net additions: new variant + 8-line doc comment + 1 unit test (Display + pattern match); loader changed from `format!()` to typed construction; 2 pre-existing tests gained a `match &err { ... }` block to assert on the new variant.

## Test Totals

| Bucket | Result |
|---|---|
| T0 fmt + clippy | PASS (0 warnings) |
| T1 chronos-store lib | 60/60 (+1 m9-08 test: `error::tests::test_schema_too_new_variant_display_and_fields`) |
| T2 chronos-services lib | 263/263 (unchanged) |
| T2 chronos-cli integration | 2/2 (unchanged) |

Sandbox tests not warranted: the change is a typed error variant + a 2-line swap in the loader's reject branch; no MCP/probe plumbing touched, no session/lifecycle/serialization surface changes. Sandbox smoke would only exercise the path if a future v2 wire-format break lands, which is out of scope.

## Findings Closed

| ID | Cluster | Severity | Closed by |
|---|---|---|---|
| FIND-M9-01-DV-COUP-02 | coupling | LOW P3 | R1 (dedicated `StoreError::SchemaTooNew` variant + loader wiring + Display/field pattern-match tests) |

Verdict: **PASS** · 1/1 finding closed.

## Out of scope (still backlog)

- `cc-001-god-module` (`counterexample_storage.rs` at ~2.6K LoC) — design-required split. After this fix the file is +27 lines (2650 from 2623 post-m9-07), growth trend unchanged.
- `cc-004-implicit-io-toctou` (`save()` read-then-write) — concurrency design required.
- `m9-02 R1-R8` — pre-existing disclosures, deferred.

## Note on auto-mode continuation

m9-08 was selected as the next safe B-direct cycle after m9-07 closed FIND-M9-01-DV-COUP-01. The lesson learned from m9-07 applies again: the **debt-report** is the authority on the finding semantics; the prior `verify-report` m9-06 classification ("requires list-side change") was over-scoped. The debt-report remediation reads as a single B-direct task ("add a dedicated error variant"), and the trigger condition ("first real v2 bump") has been satisfied since m9-01 shipped schema versions `[1, 2, 3]`.
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
