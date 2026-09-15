# Archive Manifest — m9-92-cc34-cc42-cleanup

## Identification

| Field | Value |
|---|---|
| Cycle | m9-92-cc34-cc42-cleanup |
| Path | B-direct (vault-only hardening) |
| Branch | chore/m9-92-cc34-cc42-cleanup |
| Date | 2026-09-14 |
| Base SHA | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d |
| Head SHA | `6900395d088ff89e7ee67c3260a244b3d864e4e2` |
| Merge SHA | 217f80fdca54888a9ead48a50d38146604b00d0d |
| Remote tag | v0.7.94 |
| Tag peel SHA | 6900395d088ff89e7ee67c3260a244b3d864e4e2 |

## Summary

Vault-only B-direct hardening cycle. Closes 3 pre-existing drift lines
that surfaced after m9-91 closure:

1. **CC#34 A1**: `m9-89-cascade-cc-cleanup-m9-77-87/change-entry.md` was missing a `## Cross-check` section. **FIXED**: added section.
2. **CC#34 A2**: `m9-90-stale-branches-cleanup/change-entry.md` was missing a `## Cross-check` section. **FIXED**: added section.
3. **CC#42 A**: `m9-90-stale-branches-cleanup/release-receipt.md` stored `Remote tag_peel = 2184975a` but the immutable `v0.7.92` tag now points at `5cdb4e1` (after m9-90's SHA-256 fixpoint cascade advanced the tag). **FIXED**: re-anchored to 5cdb4e1a across 5 vault artifacts.

Also re-anchored m9-90 documented SHAs from cycle source (2184975a) to
immutable v0.7.92 tag location (5cdb4e1a) to satisfy CC#3
era-awareness and match m9-89's pattern.

No Rust source touched, no tests required (T0 only).

## Drift delta

| CC | Before | After |
|---|---|---|
| CC#34 A1 | 1 | 0 |
| CC#34 A2 | 1 | 0 |
| CC#42 A | 1 | 0 |
| **Total** | **3** | **0** |

## Files changed

| Bucket | Files | Notes |
|---|---|---|
| m9-89 change-entry | 1 modified | +12 lines (new `## Cross-check` section) |
| m9-90 change-entry | 1 modified | +12 lines (new `## Cross-check` section) |
| m9-90 apply-checkpoint | 1 modified | head_sha + main_sha + remote_tag_peel re-anchored 2184975a → 5cdb4e1a |
| m9-90 release-receipt | 1 modified | Head SHA + Remote tag_peel updated |
| m9-90 release-report | 1 modified | Status + Cross-checks updated |
| m9-90 merge-receipt | 1 modified | Head SHA + Main SHA updated |
| m9-90 archive-manifest | 1 modified | Head SHA single-line + re-anchor blockquote |
| m9-92 cycle-artifacts | 7 added | apply-checkpoint + 6 receipts/reports |
| m9-92 knowledge artifacts | 5 added | proposal + spec + tasks + exploration + change-entry |
| cycles/index.md | 1 modified | m9-92 row + Total 91 → 92 |
| terms/index.md | 1 modified | Last archive = m9-92 |
| archive-manifests (CC#4) | 12 modified | SHA-256 fixpoint cascade (cycles/index.md + terms/index.md) |

**Total**: 5 vault-edit files + 12 cycle artifacts + 2 index files.

## Cross-checks

- `bash scripts/check_vault_drift.sh`: clean (0 drift lines).
- `python3 scripts/regen_manifest_index_shas.py --check`: clean.
- `apply-checkpoint.peel_match` == `true` (m9-92 + re-anchored m9-90).
- `apply-checkpoint.head_sha == release-receipt.Head SHA == 6900395d`.
- `Remote tag` v0.7.94 peel: `6900395d` (cycle-artifacts; will move
  through SHA-cascade fixpoint per CC#42 workaround).
- `apply-checkpoint.status` == `"CLOSED"`.
- `apply-checkpoint.archive_status` == `"complete"`.
- `cycles/index.md` Total cycles = 92 (matches actual folder count).
- `terms/index.md` Last archive = m9-92-cc34-cc42-cleanup.
- `cargo fmt --all -- --check`: not required (no Rust touched).
- m9-90 SHAs re-anchored from 2184975a to 5cdb4e1a across 5 vault
  artifacts; original cycle source retained in `tag_peel_note`.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-92-cc34-cc42-cleanup/apply-checkpoint.json` | `7878fd2cf43d6ba4bc266712778bad114a70e021c8c919cb47827cb5bd553a32` |
| implementation-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-92-cc34-cc42-cleanup/implementation-receipt.md` | `72c48d2320e6e332463a36ecaca0eb7a16eac49bd40f2a9ac3878487356a1063` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-92-cc34-cc42-cleanup/merge-receipt.md` | `9c5dc8b57a093b35adde6816addd7cae07ec7dbdf8ec9a5ad830f1bd977e3e7e` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-92-cc34-cc42-cleanup/release-receipt.md` | `e98d5f535fadee82d79f52b0625010a2d03c3504e99647589e0ddc0c4dfe696e` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-92-cc34-cc42-cleanup/release-report.md` | `324f5742b1aadb831e95b9914d699870026b55f6f1900d49837581b9725598c5` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-92-cc34-cc42-cleanup/verify-findings.json` | `1726c795ae22e05ecca9359089264b8c3534841ee38894230a854ac31bc037b0` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-92-cc34-cc42-cleanup/verify-report.md` | `3d9b7fdaa4436a251a7466ddf1bd6967b715ecc9d3b54ef3a576a6cdc6ee20f7` |
| proposal | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-92-cc34-cc42-cleanup/proposal.md` | `59087761f8e2a30eab970fa3a4c2aa4cd35bf5243a28996c4daf6ce00f92a483` |
| spec | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-92-cc34-cc42-cleanup/spec.md` | `40dfa73f90a699a310f6248c0aaa3258915c2920e4dad712b0e5f5c4ed93e4dd` |
| tasks | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-92-cc34-cc42-cleanup/tasks.md` | `58301e54e19cce5a2f66eb4c9c4585c8aa5508e5508d08bc379300c0c8373c19` |
| exploration-report | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-92-cc34-cc42-cleanup/exploration-report.md` | `dd1ddb46a8c9c318c82c3ce5d3c907ab604b9c45d1ce5c407bb8ade689e61ad0` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-92-cc34-cc42-cleanup/change-entry.md` | `d7080e614578907265e045fdc510acf1919363e7a74455ef313b69b63d873255` |

## Evidence bindings

The Artifact index above provides the SHA-256 binding between each
released artifact and the file content at archive time. Readers can
verify each binding with:

```bash
sha256sum <path>  # compare against the SHA-256 listed in the index
```

**Commit provenance:**

| Artifact | Bound to |
|---|---|
| Cycle source | `e480508c372893ae3e4a9301e708b23c61a312ae` (m9-92: vault edits) |
| Cycle artifacts | `6900395d088ff89e7ee67c3260a244b3d864e4e2` (m9-92: cycle artifacts) |
| Cascade commit | `f4ff58bc576b95d4416135503bb19dba88f0c71c` (m9-92: align artifacts) |
| Merge commit | `217f80fdca54888a9ead48a50d38146604b00d0d` (--no-ff merge) |
| Tag v0.7.94 | `6900395d` (pre-created at cycle-artifacts per CC#42 workaround) |

**Re-anchor provenance (m9-90 SHAs):**

m9-90 documented SHAs were re-anchored from cycle source (2184975a)
to immutable post-cascade tag location (5cdb4e1a) across 5 vault
artifacts. The original cycle source 2184975a is retained in
`tag_peel_note` and the archive-manifest blockquote for historical
record.
