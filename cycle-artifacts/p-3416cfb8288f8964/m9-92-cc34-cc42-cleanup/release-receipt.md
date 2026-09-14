# Release Receipt — m9-92-cc34-cc42-cleanup

## Identification

| Field | Value |
|---|---|
| Cycle | m9-92-cc34-cc42-cleanup |
| Path | B-direct |
| Branch | chore/m9-92-cc34-cc42-cleanup |
| Date | 2026-09-14 |

## Release details

| Field | Value |
|---|---|
| Base SHA | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d |
| Head SHA | e480508c372893ae3e4a9301e708b23c61a312ae |
| Main SHA | e480508c372893ae3e4a9301e708b23c61a312ae |
| Remote tag | v0.7.94 |
| Remote tag_peel | e480508c372893ae3e4a9301e708b23c61a312ae |
| Peel match | true |

## SHAs (canonical table)

| Field | Value |
|---|---|
| Branch | chore/m9-92-cc34-cc42-cleanup |
| Date | 2026-09-14 |
| Base SHA | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d |
| Head SHA | e480508c372893ae3e4a9301e708b23c61a312ae |
| Remote tag | v0.7.94 |
| Remote tag_peel | e480508c372893ae3e4a9301e708b23c61a312ae |
| Peel match | true |

## Release notes

- Vault-only B-direct cycle. No Rust source code touched.
- Tag `v0.7.94` was pre-created at the cycle-artifacts commit SHA and moved through merge + SHA-cascade fixpoint commits per the CC#42 fixpoint-cascade workaround (m9-83 handoff).
- **Closes 3 pre-existing drift lines** introduced by m9-91 closure:
  - CC#34 A1: `m9-89-cascade-cc-cleanup-m9-77-87/change-entry.md` now has `## Cross-check` section.
  - CC#34 A2: `m9-90-stale-branches-cleanup/change-entry.md` now has `## Cross-check` section.
  - CC#42 A: `m9-90-stale-branches-cleanup/release-receipt.md` `Remote tag_peel` re-anchored to 5cdb4e1 (immutable v0.7.92 tag location).
- **Re-anchored m9-90 documented SHAs** from cycle-artifacts (2184975a) to immutable tag location (5cdb4e1) to satisfy CC#3 era-awareness and match m9-89's pattern.
- **Net drift delta**: 3 → 0 (CC#34, CC#42 all clean post-cycle).

## Cross-checks

- `bash scripts/check_vault_drift.sh`: clean (0 drift lines reported).
- `python3 scripts/regen_manifest_index_shas.py --check`: clean (no stale rows).
- `cargo fmt --all -- --check`: not required (no Rust touched).
- `apply-checkpoint.peel_match == true`.
- `apply-checkpoint.head_sha == release-receipt.Head SHA == merge-receipt.Head SHA == e480508c372893ae3e4a9301e708b23c61a312ae`.
