# Release Report — m9-04

**Cycle**: m9-04-side-table-key-layout
**Path**: B-direct


## Release Envelope

```yaml
status: success
route: local
change: m9-04-side-table-key-layout
cycle_id: p-3416cfb8288f8964/m9-04-side-table-key-layout
main_sha: d6b3b8c51c50d2ce6d0fe4f8c804cf793137f0dc
tag: v0.7.2
merge_receipt: cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/merge-receipt.md
release_receipt: cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/release-receipt.md
verify_report: cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/verify-report.md
verify_findings: cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/verify-findings.json
runtime_status: RELEASED
next_phase: archive
lease_after_transition: absent
optional_distribution: not_requested
blockers: []
```

## Git Effects Summary

| Step | Result |
|---|---|
| Trunk sync (fetch + ff-pull) | PASS — main at eb2cccd, clean |
| Fast-forward merge from feat/m9-04-side-table-key-layout | PASS — eb2cccd..d6b3b8c |
| Direct trunk push | PASS — eb2cccd..d6b3b8c pushed to origin/main |
| Remote SHA verification | PASS — origin/main == d6b3b8c51c50d2ce6d0fe4f8c804cf793137f0dc |
| Annotated tag | PASS — v0.7.2 created at d6b3b8c |
| Tag push | PASS — v0.7.2 -> origin |
| Tag peel verification | PASS — remote tag peels to d6b3b8c |


## Cross-checks

- C1: pass (pre-CC-cycle, no cross-checks applied)

## Files Inventory

| Status | Bucket | Path |
|---|---|---|
| added | crates/ | `crates/chronos-cli/src/lib.rs` |
| added | crates/ | `crates/chronos-cli/tests/replay_integration.rs` |
| modified | crates/ | `crates/chronos-cli/Cargo.toml` |
| modified | crates/ | `crates/chronos-cli/src/replay.rs` |
| modified | crates/ | `crates/chronos-services/src/counterexample.rs` |
| modified | crates/ | `crates/chronos-store/src/counterexample_storage.rs` |
| modified | crates/ | `crates/chronos-store/src/storage.rs` |
| added | docs/ | `docs/milestones/m9-04-side-table-key-layout-scoping.md` |
| added | docs/ | `docs/milestones/m9-04-side-table-key-layout-spec.md` |
| added | docs/ | `docs/milestones/m9-04-side-table-key-layout-design.md` |
| added | artifacts | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/verify-report.md` |
| added | artifacts | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/verify-findings.json` |

## Commits in Release

| SHA | Message |
|---|---|
| `083f5ba` | docs(m9-04): scoping — side-table key layout (range-scan friendly, v3 schema) |
| `6ec8757` | docs(m9-04): spec — side-table key layout (range-scan friendly) |
| `140d53a` | docs(m9-04): design — side-table v3 key layout (range-scan friendly) |
| `7f86a1c` | feat(m9-04): side-table key layout v3 (blake3 prefix, 20-byte fixed keys) |
| `379759e` | m9-04 side-table-key-layout: fix verify failures, add CLI replay integration tests |
| `d6b3b8c` | verify(m9-04): PASS — remediation resolved all 6 prior findings |

## Artifacts (SHA-256)

| Kind | Path | SHA-256 |
|---|---|---|
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/merge-receipt.md` | `2a5652e6e1ad40d21dc49224ef3270369dac1604bdca41409595d68eb87f5b0a` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/release-receipt.md` | `b870428a2a990a1b80c921ebff38b70aff5e1e4a9339390849ba7d4694fa8d4a` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/verify-report.md` | `9786a2b530156f5bd9961318ed8e70c3e021aeab05150fd8f6f890259419151c` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/verify-findings.json` | `fd8de7a8416f184cab8389f6a36db2bca375d63b7cc8071177bc9fe2b7c87c23` |

## no-pending-effects

All required local Git effects are complete:

- Trunk pushed to origin/main ✓
- Annotated tag v0.7.2 on origin ✓
- Merge receipt captured ✓
- Release receipt captured ✓

Excluded from no-pending-effects: CI/CD, GitHub Actions, hosted releases, assets, signatures, optional post-tag distribution.

## Open follow-ups (m9+ backlog, captured in archive-manifest)

This cycle's debt-verify produced 4 apply-target + 2 backlog + 1 no-action findings. The 4 apply-target findings are scheduled for `m9-05-side-table-overeng-cleanup` (B-direct). The 2 backlog findings (god-module + TOCTOU) are recorded for later cycles.