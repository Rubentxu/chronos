# Release Report — m9-06

## Release Envelope

```yaml
status: success
route: local
change: m9-06-known-schema-versions-invariant
cycle_id: p-3416cfb8288f8964/m9-06-known-schema-versions-invariant
main_sha: 3383905d93ac66b6e90b6de8cea760c1ad4e2c99
tag: v0.7.4
merge_receipt: cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/merge-receipt.md
release_receipt: cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/release-receipt.md
verify_report: cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/verify-report.md
runtime_status: RELEASED
next_phase: archive
lease_after_transition: absent
optional_distribution: not_requested
blockers: []
```

## Git Effects Summary

| Step | Result |
|---|---|
| Trunk sync (fetch + ff-pull) | PASS — main at 9d2c676, clean |
| Direct commit (single-file trivial B-direct, no separate branch) | PASS — 9d2c676..3383905 |
| Direct trunk push | PASS — 9d2c676..3383905 pushed to origin/main |
| Remote SHA verification | PASS — origin/main == 3383905d93ac66b6e90b6de8cea760c1ad4e2c99 |
| Annotated tag | PASS — v0.7.4 created at 3383905 |
| Tag push | PASS — v0.7.4 -> origin |
| Tag peel verification | PASS — remote tag peels to 3383905 |

## Verification Summary

Source: `cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/verify-report.md`

| Verdict | Mode | Path | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|
| PASS | light-verify inline | B-direct | T0/T1/T2 all green (chronos-store 57/57, chronos-services 263/263, chronos-cli 2/2) | 0 | 0 |

### Behavioral Compliance (2/2 spec scenarios pinned)

| Finding | Description | Status |
|---|---|---|
| m9-01-R4 / R1 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` no longer marked dead-code in production build | COMPLIANT |
| FIND-M9-01-DV-OE-01 / R2 | `CURRENT_BUNDLE_SCHEMA_VERSION ∈ KNOWN_BUNDLE_SCHEMA_VERSIONS` enforced at compile time | COMPLIANT |

## Files Inventory

| Status | Bucket | Path |
|---|---|---|
| modified | crates/ | `crates/chronos-store/src/counterexample_storage.rs` |
| added | artifacts | `cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/verify-report.md` |

## Commits in Release

| SHA | Message |
|---|---|
| `1feeab4` | fix(m9-06): remove stale #[allow(dead_code)] on KNOWN_BUNDLE_SCHEMA_VERSIONS; add compile-time invariant + test (closes m9-01-R4, FIND-M9-01-DV-OE-01) |
| `3383905` | docs(m9-06): light-verify evidence (R1+R2 PASS, 2/2 findings closed) |

## Artifacts (SHA-256)

| Kind | Path | SHA-256 |
|---|---|---|
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/merge-receipt.md` | (computed post-write) |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/release-receipt.md` | (computed post-write) |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/verify-report.md` | (computed post-write) |

## no-pending-effects

All required local Git effects are complete:

- Trunk pushed to origin/main ✓
- Annotated tag v0.7.4 on origin ✓
- Merge receipt captured ✓
- Release receipt captured ✓

Excluded from no-pending-effects: CI/CD, GitHub Actions, hosted releases, assets, signatures, optional post-tag distribution.

## Open follow-ups (still m9+ backlog)

| ID | Cluster | Severity | Título |
|---|---|---|---|
| cc-001-god-module | coupling | MEDIUM | `counterexample_storage.rs` at ~2.5K LoC; 5 distinct concerns |
| cc-004-implicit-io-toctou | coupling | LOW | `save_bundle_record_and_events` opens read-then-write |
| FIND-M9-01-DV-COUP-01 | coupling | MEDIUM | Duplicated `schema_version` on record + summary |
| FIND-M9-01-DV-COUP-02 | coupling | LOW | List/load policy asymmetry |
| m9-02 R1-R8 | various | — | See m9-02 change-entry for details |