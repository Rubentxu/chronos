# Release Report: m9-07-coup-01-invariant-assertion

## Release Envelope

```yaml
status: success
route: local
change: m9-07-coup-01-invariant-assertion
cycle_id: p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion
main_sha: 3edb01f0a17c3ac8877df1e7868d4e3cca374217
tag: v0.7.5
merge_receipt: cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/merge-receipt.md
release_receipt: cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/release-receipt.md
verify_report: cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/verify-report.md
runtime_status: RELEASED
next_phase: archive
lease_after_transition: absent
optional_distribution: not_requested
blockers: []
```

## Git Effects Summary

| Step | Result |
|---|---|
| Trunk sync (fetch + ff-pull) | PASS — main at aa96e5a, clean |
| Direct commits on main (no separate branch) | PASS — 3edb01f fix + 5c5b3ec docs |
| Direct trunk push | PASS — aa96e5a..5c5b3ec pushed to origin/main |
| Remote SHA verification | PASS — origin/main == 5c5b3ec (docs head); tag peeled to 3edb01f (fix head) |
| Annotated tag | PASS — v0.7.5 created at 3edb01f0a17c3ac8877df1e7868d4e3cca374217 |
| Tag push | PASS — v0.7.5 -> origin |
| Tag peel verification | PASS — remote tag peels to 3edb01f |

## Verification Summary

Source: `cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/verify-report.md`

| Verdict | Mode | Path | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|
| PASS | light-verify inline | B-direct | T0/T1/T2 (store + cli + services) all green | 0 | 0 |

### Behavioral Compliance (1/1 spec scenario pinned)

| Finding | Description | Status |
|---|---|---|
| R1 / FIND-M9-01-DV-COUP-01 | loader rejects records whose envelope `schema_version` disagrees with `summary.schema_version` | COMPLIANT |

## Files Inventory

| Status | Bucket | Path |
|---|---|---|
| modified | crates/ | `crates/chronos-store/src/counterexample_storage.rs` |
| added | artifacts | `cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/verify-report.md` |

## Commits in Release

| SHA | Message |
|---|---|
| `3edb01f` | fix(m9-07): loader rejects envelope/summary schema_version mismatch (closes FIND-M9-01-DV-COUP-01) |
| `5c5b3ec` | docs(m9-07): light-verify evidence (R1 PASS, FIND-M9-01-DV-COUP-01 closed) |

## Artifacts (SHA-256)

| Kind | Path | SHA-256 |
|---|---|---|
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/merge-receipt.md` | (computed post-write) |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/release-receipt.md` | (computed post-write) |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/verify-report.md` | (computed post-write) |

## no-pending-effects

All required local Git effects are complete:

- Trunk pushed to origin/main ✓
- Annotated tag v0.7.5 on origin ✓
- Merge receipt captured ✓
- Release receipt captured ✓

Excluded from no-pending-effects: CI/CD, GitHub Actions, hosted releases, assets, signatures, optional post-tag distribution.

## Open follow-ups (m9+ backlog, NOT closed by m9-07)

| ID | Cluster | Severity | Title |
|---|---|---|---|
| FIND-M9-01-DV-COUP-02 | coupling | LOW P3 | list/load policy asymmetry shipped as an error-kind overload; needs list-side change with a dedicated `SchemaTooNew` error variant at the next wire-format break |
| cc-001-god-module | coupling | MEDIUM | `counterexample_storage.rs` at ~2.6K LoC (+57 lines this cycle, +114 lines m9-04..m9-07 net); 5 distinct concerns (split into keys/records/persistence/schema-version/v2-legacy modules) |
| cc-004-implicit-io-toctou | coupling | LOW | `save_bundle_record_and_events` opens read-then-write; pre-existing TOCTOU pattern (m9-02 R7) |
| m9-02 R1-R8 | disclosure | various | pre-existing disclosures, deferred |
