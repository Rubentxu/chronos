# Release Receipt — m9-90-stale-branches-cleanup

## Identification

| Field | Value |
|---|---|
| Cycle | m9-90-stale-branches-cleanup |
| Path | B-direct |
| Branch | chore/m9-90-stale-branches-cleanup |
| Date | 2026-09-14 |

## Release details

| Field | Value |
|---|---|
| Base SHA | 20e2649822c0c24509ff6419a48fe3113591ecf0 |
| Head SHA | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d *(re-anchored to immutable v0.7.92 tag location by m9-92; cycle source 2184975a)* |
| Main SHA | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d *(post-cascade HEAD; aligns with immutable tag)* |
| Remote tag | v0.7.92 |
| Remote tag_peel | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d *(immutable; CC#42 fixpoint-cascade workaround applied by m9-92)* |
| Peel match | true |

## SHAs (canonical table)

| Field | Value |
|---|---|
| Branch | chore/m9-90-stale-branches-cleanup |
| Date | 2026-09-14 |
| Base SHA | 20e2649822c0c24509ff6419a48fe3113591ecf0 |
| Head SHA (cycle-artifacts) | 2184975a93b43ea1bbd2681dead79dfb4476fef7 *(original cycle source; re-anchored to 5cdb4e1a for tag alignment)* |
| Remote tag | v0.7.92 |
| Remote tag_peel | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d (immutable; CC#42 fixpoint-cascade workaround applied by m9-92) |
| Peel match | true |

## Release notes

- Vault-only B-direct cycle. No Rust source code touched.
- Tag `v0.7.92` was pre-created at the cleanup commit SHA
  (`13f7084...`) and moved to the merge commit per the CC#42
  fixpoint-cascade workaround (m9-83 handoff).
- **CC#42 fixpoint-cascade workaround re-applied (m9-92)**: the
  immutable `v0.7.92` tag was later advanced to
  `5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d` (the post-cascade
  HEAD) when the SHA-256 fixpoint cascade landed at push time.
  m9-92 re-aligns the stored `Remote tag_peel` to that immutable
  location, matching the pattern established by m9-89 (m9-89 was
  re-aligned to v0.7.91's actual peel `47a10f8` for the same
  reason). Pushed tags are immutable; the receipts must follow.
- Cross-checks satisfied (at apply-time):
  - `bash scripts/check_vault_drift.sh`: CC#46 + CC#53 clean.
  - `python3 scripts/clean_m9_90_stale_branches.py --dry-run`: 0
    candidates (idempotent — nothing left to delete).
  - `cargo fmt --all -- --check`: clean.
  - `cargo clippy --workspace --all-targets -- -D warnings`: clean.
  - Recovery log: `scripts/branches-deleted-m9-90.log`.
