# Verification Report: m9-07-coup-01-invariant-assertion

## Subject

| Base | Head | CWD | Verified at |
|---|---|---|---|
| `aa96e5a` | `3edb01f` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-12T06:53:30+02:00 |

`git status --porcelain` after the fix commit is empty (only `.jcode/` ignored). HEAD is pinned at `3edb01f`. Subject identity gate: **PASS**.

## Files Inventory

Source: `git diff --stat aa96e5a..3edb01f`.

| Bucket | Added | Modified | Deleted | Renamed |
|---|---:|---:|---:|---:|
| `crates/` | 0 | 1 | 0 | 0 |

| Status | Bucket | Path |
|---|---|---|
| modified | crates/ | `crates/chronos-store/src/counterexample_storage.rs` |

## Summary

| Verdict | Mode | Path | Required scenarios | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|---|
| **PASS** | light-verify inline | B-direct | 1 spec scenario (envelope↔summary schema_version invariant at load time) | T0, T1, T2 all green | 0 | 0 |

## Behavioral Compliance

| # | Scenario | Production Path | Test | Status | Evidence |
|---|---|---|---|---|---|
| R1 | `load_counterexample_bundle` rejects records whose envelope `schema_version` disagrees with `summary.schema_version` | new 5-line `if` block (lines 904-910) returns `StoreError::Serialization` with a `"disagrees with summary"` message; placed after the existing future-version check (line 889) so it only fires on in-range records | test `m9_07_loader_rejects_envelope_summary_version_mismatch` injects envelope=3/summary=1 via the m9-05 chokepoint `insert_bundle_record_for_test` and asserts rejection | **COMPLIANT** | T1 58/58 (m9-07 test passes); no existing test regressed |

The fix is the textbook B-direct remediation prescribed by the debt-report finding itself:

> **Remediation (backlog):** add a 3-line loader assertion `record.schema_version == summary.schema_version`, or drop the nested field at the next wire-format break.

m9-07 takes the loader-assertion path (option A). Option B (drop the field) is deferred to the next wire-format break, which is not on the m9+ roadmap.

## Production Readiness

| Gate | Status | Evidence |
|---|---|---|
| Errors / recovery | PASS | New error is `StoreError::Serialization` with a precise message that includes both envelope and summary version numbers; surfaces the drift to the caller, never silently passes |
| State / data integrity | PASS | `save()` already canonicalizes both fields to `CURRENT_BUNDLE_SCHEMA_VERSION` (lines 577-578), so legitimate writes never trip the new check; the new branch only fires on records a hand-constructed tool produced with a mismatch |
| Backward compatibility | PASS | Pre-m9-07 records persisted by older chronos-store have both fields set to the same value (either both `default_schema_version()` or both canonicalized by save); no existing fixture triggers the new branch (T1 confirms zero regressions on the 57 pre-existing tests) |
| Resource cleanup | PASS | Pure synchronous check; no allocation beyond the error-format string |

## Source Diff Summary

```
crates/chronos-store/src/counterexample_storage.rs | +58 -1
1 file changed, 58 insertions(+), 1 deletion(-)
```

Net additions: 5-line production guard + 1 doc comment + 1 test function (33 lines incl. setup + assertions) + 2-line pre-existing `assert!` reformat that `cargo fmt` repaired.

## Test Totals

| Bucket | Result |
|---|---|
| T0 fmt + clippy | PASS (0 warnings) |
| T1 chronos-store lib | 58/58 (+1 m9-07 test) |
| T2 chronos-services lib | 263/263 (unchanged) |
| T2 chronos-cli integration | 2/2 (unchanged) |

Sandbox tests not warranted: the change is a single synchronous guard in a pure-storage method; no MCP/probe plumbing touched, no session/lifecycle/serialization surface changes. Sandbox smoke would only exercise the path if a future v2 wire-format break lands, which is out of scope.

## Findings Closed

| ID | Cluster | Severity | Closed by |
|---|---|---|---|
| FIND-M9-01-DV-COUP-01 | coupling | MEDIUM P2 | R1 (loader invariant + test pin) |

Verdict: **PASS** · 1/1 finding closed.

## Out of scope (still backlog)

- `FIND-M9-01-DV-COUP-02` (list/load policy asymmetry) — requires list-side change with a dedicated `SchemaTooNew` error variant. First candidate: at the v4 schema bump, not the m9 cycle. Still debt.
- `m9-02 R1-R8` — pre-existing disclosures, deferred.
- `cc-001-god-module` (`counterexample_storage.rs` at ~2.5K LoC) — design-required split. After this fix the file is +57 lines (2620 from 2563 pre-fix), growth trend unchanged.
- `cc-004-implicit-io-toctou` (`save()` read-then-write) — concurrency design required.

## Note on auto-mode continuation

m9-07 was selected as the next safe B-direct cycle after a negative-result exploration (recorded in m9-06's apply-checkpoint audit note `next-up-audit-2026-09-12`). The debt-report prescribed a 3-line loader assertion explicitly; the only design judgment required was picking option A (loader assertion) over option B (drop nested field at next wire break), which is a documented B-direct call: take the cheaper fix today, defer the structural fix to the natural break point.
