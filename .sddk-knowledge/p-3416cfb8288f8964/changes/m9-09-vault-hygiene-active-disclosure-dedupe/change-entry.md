# Change: m9-09 vault hygiene active disclosure dedupe

## Summary

Drift closure cycle for this milestone.


## Subject

- cycle: m9-09
- route: B-direct
- base_sha: `c183ad1822423419ffb7d5f92a8d5ac46423d653`
- head_sha: `07e731d61424160c4f67d769db00a171837ba62c`

## Commits

| SHA | Subject |
|---|---|
| `07e731d` | fix(m9-09): remove duplicate m9-01-R4 from active disclosures (already terminated by m9-06) |
| `bc94a33` | docs(m9-09): light-verify evidence (R1 PASS, vault drift closed) |

## Scope

Trivial B-direct **vault-hygiene** fix. No Rust code changed. The change is a
single-row deletion in `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`:

- **Removed**: row for `m9-01-R4` from the "Disclosures (m9-01 scoping R1–R4)"
  active-disclosures table.
- **Reason**: `m9-01-R4` was already present in the Terminated section
  (`m9-06-known-schema-versions-invariant (v0.7.4)`). The duplicate appeared
  when the m9-06 archive phase added the terminated entry without removing
  the now-stale active entry.

This drift was discovered by an `awk | sort | uniq -c` sweep over the
terms/index.md sections. Pre-fix output showed `m9-01-R4` with count 2; all
other IDs had count 1. Post-fix: all IDs have count 1.

The semantic meaning of `m9-01-R4` ("`KNOWN_BUNDLE_SCHEMA_VERSIONS` unused;
`#[allow(dead_code)]`") has been obsolete since m9-06:

- m9-06 removed the `#[allow(dead_code)]` attribute.
- m9-06 added a `const _: () = { ... }` compile-time invariant that
  references the constant, making it no longer dead in production builds.
- m9-06 added a `m9_06_current_schema_version_is_listed_as_known` lib test
  that also references the constant.

The code change in `crates/chronos-store/src/counterexample_storage.rs` is
preserved unchanged — m9-09 only updates the vault index to reflect the
actual state of the codebase.

### Changed paths

- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` — removed 1 row from
  the active-disclosures table.

## Findings resolved

This cycle closes **no debt findings**. It is a documentation-drift
hygiene fix only. The `m9-01-R4` disclosure was already terminated by
m9-06; the change is to remove the duplicate that left the disclosure
listed in both Active and Terminated sections.

## Cross-check

Cross-check C1-C10 added to `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`.

## Files changed

- `cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/release-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-09-vault-hygiene-active-disclosure-dedupe/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-09-vault-hygiene-active-disclosure-dedupe/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Findings NOT resolved (m9+ backlog unchanged)

| ID | Cluster | Severity | Title | Status |
|---|---|---|---|---|
| m9-02 R1–R8 | disclosure | — | various scope disclosures | by-design (untouched) |
| m9-01 R1–R3 | disclosure | — | forward-compat + canonical-write disclosures | by-design (untouched) |
| m9-04 R1–R6 | disclosure | — | layout + collision + scan disclosures | by-design (untouched) |
| cc-001-god-module | coupling | MEDIUM | `counterexample_storage.rs` at ~2.6K LoC | design-required split |
| cc-004-implicit-io-toctou | coupling | LOW | `save_bundle_record_and_events` opens read-then-write | concurrency design required |
| m8-06-R4 | disclosure | — | Cross-variant existence predicate shrinking | by-design (untouched) |
| m8-04-R-hypothesis-fallback | disclosure | — | `property_target` lost in fallback reconstruction | by-design (untouched) |
