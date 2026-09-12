# Verification Report: m9-06-known-schema-versions-invariant

## Subject

| Base | Head | CWD | Verified at |
|---|---|---|---|
| `9d2c676` | `1feeab4` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-12T08:43:00+02:00 |

`git status --porcelain` shows only `.jcode/` (gitignored). HEAD is pinned at `1feeab4...`. Subject identity gate: **PASS**.

## Files Inventory

Source: `git diff --stat 9d2c676..HEAD`.

| Bucket | Added | Modified | Deleted | Renamed |
|---|---:|---:|---:|---:|
| `crates/` | 0 | 1 | 0 | 0 |

| Status | Bucket | Path |
|---|---|---|
| modified | crates/ | `crates/chronos-store/src/counterexample_storage.rs` |

## Summary

| Verdict | Mode | Path | Required scenarios | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|---|
| **PASS** | light-verify inline | B-direct | 2 spec scenarios (production-build invariant + lib test pin) | T0, T1, T2 all green | 0 | 0 |

## Behavioral Compliance

| # | Scenario | Production Path | Test | Status | Evidence |
|---|---|---|---|---|---|
| R1 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` no longer marked dead-code in production build | compile-time `const _: () = { ... }` invariant that references the constant | cargo build --workspace --all-targets (no `dead_code` warning) | **COMPLIANT** | T0 clean: 0 warnings |
| R2 | `CURRENT_BUNDLE_SCHEMA_VERSION ∈ KNOWN_BUNDLE_SCHEMA_VERSIONS` enforced at compile time | manual loop in const block (bypasses `<[u32]>::contains` non-const); `assert!` at module scope | test `m9_06_current_schema_version_is_listed_as_known` pins it for grep-ability | **COMPLIANT** | T1 57/57 (m9_06 test passes); const-eval assert fires if the invariant is violated |
| R3 | m9-04 R6 disclosure pin (`KNOWN == [1, 2, 3]`, exactly 3) remains green | same constant | `m9_04_known_bundle_schema_versions_pinned` (pre-existing) | **COMPLIANT** | T1 57/57 |

## Production Readiness

| Gate | Status | Evidence |
|---|---|---|
| Errors / recovery | PASS | The compile-time invariant catches `CURRENT_BUNDLE_SCHEMA_VERSION` drift at build time, before any code runs |
| State / data integrity | PASS | `KNOWN_BUNDLE_SCHEMA_VERSIONS` was already used by `m9_04_known_bundle_schema_versions_pinned` (test) and now is also used by the const-eval invariant (production build) |
| Resource cleanup | PASS | No runtime allocation, no handle |
| Compile-time feedback | PASS | Future schema bumps will fail to compile if the new version isn't added to `KNOWN_BUNDLE_SCHEMA_VERSIONS` |

## Source Diff Summary

```
crates/chronos-store/src/counterexample_storage.rs | +37 -1
1 file changed, 37 insertions(+), 1 deletion(-)
```

Net additions: doc comment on the constant + a compile-time invariant block + one test. The `#[allow(dead_code)]` annotation removed (1 LoC deleted).

## Test Totals

| Bucket | Result |
|---|---|
| T0 fmt + clippy | PASS (0 warnings) |
| T1 chronos-store lib | 57/57 (+1 m9-06 test) |
| T2 chronos-services lib | 263/263 (unchanged) |
| T2 chronos-cli integration | 2/2 (unchanged) |

Sandbox tests not warranted: no MCP/probe plumbing touched, no session/lifecycle/serialization changes.

## Findings Closed

| ID | Cluster | Severity | Closed by |
|---|---|---|---|
| m9-01-R4 | disclosure | — | R1 + R2 (compile-time + test pin) |
| FIND-M9-01-DV-OE-01 | overeng | LOW P3 | R1 + R2 (constant no longer speculative) |

Verdict: **PASS** · 2/2 findings closed.

## Out of scope (still backlog)

- `FIND-M9-01-DV-COUP-01` (duplicated `schema_version` on record + summary) — design decision required to pick canonical source.
- `FIND-M9-01-DV-COUP-02` (list/load policy asymmetry) — requires list-side change.
- m9-02 R1-R8 — pre-existing disclosures, deferred.
- `cc-001-god-module` (counterexample_storage.rs at ~2.5K LoC) — design-required split.
- `cc-004-implicit-io-toctou` (save() read-then-write) — concurrency design required.
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
