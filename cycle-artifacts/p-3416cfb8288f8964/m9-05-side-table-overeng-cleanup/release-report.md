# Release Report: m9-05-side-table-overeng-cleanup

## Release Envelope

```yaml
status: success
route: local
change: m9-05-side-table-overeng-cleanup
cycle_id: p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup
main_sha: 07d01d5869ff6e3ffed29315b761e6e476f3b70d
tag: v0.7.3
merge_receipt: cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/merge-receipt.md
release_receipt: cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/release-receipt.md
verify_report: cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/verify-report.md
runtime_status: RELEASED
next_phase: archive
lease_after_transition: absent
optional_distribution: not_requested
blockers: []
```

## Git Effects Summary

| Step | Result |
|---|---|
| Trunk sync (fetch + ff-pull) | PASS — main at c9e5417, clean |
| Fast-forward merge from feat/m9-05-side-table-overeng-cleanup | PASS — c9e5417..07d01d5 |
| Direct trunk push | PASS — c9e5417..07d01d5 pushed to origin/main |
| Remote SHA verification | PASS — origin/main == 07d01d5869ff6e3ffed29315b761e6e476f3b70d |
| Annotated tag | PASS — v0.7.3 created at 07d01d5 |
| Tag push | PASS — v0.7.3 -> origin |
| Tag peel verification | PASS — remote tag peels to 07d01d5 |

## Verification Summary

Source: `cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/verify-report.md`

| Verdict | Mode | Path | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|
| PASS | light-verify inline | B-direct | T0/T1/T2 (store + cli + services) all green | 0 | 0 |

### Behavioral Compliance (4/4 spec scenarios pinned)

| Finding | Description | Status |
|---|---|---|
| R1 / overeng-001 | `decode_chunk_payload` extracted and used by both loaders | COMPLIANT |
| R2 / overeng-002 | `collect_v3_keys_for_bundle` extracted; `save_bundle_record_and_events` reuses it | COMPLIANT |
| R3 / overeng-003 | `collect_bundle_chunks(events_count: u64)` — `Option` wrapper dropped | COMPLIANT |
| R4 / cc-003 | `db()` + table constants narrowed to module-private; cli test uses chokepoints | COMPLIANT |

## Files Inventory

| Status | Bucket | Path |
|---|---|---|
| added | docs/ | `docs/milestones/m9-05-side-table-overeng-cleanup-scoping.md` |
| modified | crates/ | `crates/chronos-cli/tests/replay_integration.rs` |
| modified | crates/ | `crates/chronos-store/src/counterexample_storage.rs` |
| modified | crates/ | `crates/chronos-store/src/storage.rs` |
| added | artifacts | `cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/verify-report.md` |

## Commits in Release

| SHA | Message |
|---|---|
| `232e2d2` | docs(m9-05): scoping — side-table overeng cleanup (4 apply-target findings) |
| `fa0ca90` | fix(m9-05): close 4 apply-target debt findings — extract decode_chunk_payload, collect_v3_keys_for_bundle; drop events_count Option wrapper; narrow pub visibility with chokepoints |
| `07d01d5` | docs(m9-05): light-verify evidence (R1-R4 PASS, 4/4 findings closed) |

## Artifacts (SHA-256)

| Kind | Path | SHA-256 |
|---|---|---|
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/merge-receipt.md` | (computed post-write) |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/release-receipt.md` | (computed post-write) |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/verify-report.md` | (computed post-write) |
| scoping | `docs/milestones/m9-05-side-table-overeng-cleanup-scoping.md` | (computed post-write) |

## no-pending-effects

All required local Git effects are complete:

- Trunk pushed to origin/main ✓
- Annotated tag v0.7.3 on origin ✓
- Merge receipt captured ✓
- Release receipt captured ✓

Excluded from no-pending-effects: CI/CD, GitHub Actions, hosted releases, assets, signatures, optional post-tag distribution.

## Open follow-ups (m9+ backlog, NOT closed by m9-05)

| ID | Cluster | Severity | Título |
|---|---|---|---|
| cc-001-god-module | coupling | MEDIUM | `counterexample_storage.rs` at ~2.5K LoC; 5 distinct concerns (split into keys/records/persistence/schema-version/v2-legacy modules) |
| cc-004-implicit-io-toctou | coupling | LOW | `save_bundle_record_and_events` opens read-then-write; pre-existing TOCTOU pattern (m9-02 R7) |