# Release Report — m9-08

**Cycle**: m9-08-list-load-schema-error-variant
**Path**: B-direct


## Release Envelope

```yaml
status: success
route: local
change: m9-08-list-load-schema-error-variant
cycle_id: p-3416cfb8288f8964/m9-08-list-load-schema-error-variant
main_sha: d89862bbe67256cf6274be1d71ee7f8857cb8808
tag: v0.7.6
merge_receipt: cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/merge-receipt.md
release_receipt: cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/release-receipt.md
verify_report: cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/verify-report.md
runtime_status: RELEASED
next_phase: archive
lease_after_transition: absent
optional_distribution: not_requested
blockers: []
```

## Git Effects Summary

| Step | Result |
|---|---|
| Trunk sync (fetch + ff-pull) | PASS — main at 5055396, clean |
| Direct commits on main (no separate branch) | PASS — d89862b fix + cb47fe8 docs |
| Direct trunk push | PASS — 5055396..cb47fe8 pushed to origin/main |
| Remote SHA verification | PASS — origin/main == cb47fe8 (docs head); tag peeled to d89862b (fix head) per m9-04..m9-07 convention |
| Annotated tag | PASS — v0.7.6 created at d89862bbe67256cf6274be1d71ee7f8857cb8808 |
| Tag push | PASS — v0.7.6 -> origin |
| Tag peel verification | PASS — remote tag peels to d89862b |


## Cross-checks

- C1: pass (pre-CC-cycle, no cross-checks applied)

## Files Inventory

| Status | Bucket | Path |
|---|---|---|
| modified | crates/ | `crates/chronos-store/src/error.rs` |
| modified | crates/ | `crates/chronos-store/src/counterexample_storage.rs` |
| added | artifacts | `cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/verify-report.md` |

## Commits in Release

| SHA | Message |
|---|---|
| `d89862b` | fix(m9-08): dedicated StoreError::SchemaTooNew variant (closes FIND-M9-01-DV-COUP-02) |
| `cb47fe8` | docs(m9-08): light-verify evidence (R1 PASS, FIND-M9-01-DV-COUP-02 closed) |

## Artifacts (SHA-256)

| Kind | Path | SHA-256 |
|---|---|---|
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/merge-receipt.md` | (computed post-write) |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/release-receipt.md` | (computed post-write) |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/verify-report.md` | (computed post-write) |

## no-pending-effects

All required local Git effects are complete:

- Trunk pushed to origin/main ✓
- Annotated tag v0.7.6 on origin ✓
- Merge receipt captured ✓
- Release receipt captured ✓

Excluded from no-pending-effects: CI/CD, GitHub Actions, hosted releases, assets, signatures, optional post-tag distribution.

## Open follow-ups (m9+ backlog, NOT closed by m9-08)

| ID | Cluster | Severity | Title |
|---|---|---|---|
| cc-001-god-module | coupling | MEDIUM | `counterexample_storage.rs` at ~2.6K LoC (+27 lines this cycle, +141 net m9-04..m9-08); 5 distinct concerns (split into keys/records/persistence/schema-version/v2-legacy modules) |
| cc-004-implicit-io-toctou | coupling | LOW | `save_bundle_record_and_events` opens read-then-write; pre-existing TOCTOU pattern (m9-02 R7) |
| m9-02 R1-R8 | disclosure | various | pre-existing disclosures, deferred |
