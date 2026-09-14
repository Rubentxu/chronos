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
| Head SHA | 2184975a93b43ea1bbd2681dead79dfb4476fef7 |
| Main SHA | 2184975a93b43ea1bbd2681dead79dfb4476fef7 |
| Remote tag | v0.7.92 |
| Remote tag_peel | 2184975a93b43ea1bbd2681dead79dfb4476fef7 |
| Peel match | 2184975a93b43ea1bbd2681dead79dfb4476fef7 (will be True after tag move) |

## SHAs (canonical table)

| Field | Value |
|---|---|
| Branch | chore/m9-90-stale-branches-cleanup |
| Date | 2026-09-14 |
| Base SHA | 20e2649822c0c24509ff6419a48fe3113591ecf0 |
| Head SHA | 2184975a93b43ea1bbd2681dead79dfb4476fef7 |
| Remote tag | v0.7.92 |
| Remote tag_peel | 2184975a93b43ea1bbd2681dead79dfb4476fef7 |
| Peel match | 2184975a93b43ea1bbd2681dead79dfb4476fef7 (will be True after tag move) |

## Release notes

- Vault-only B-direct cycle. No Rust source code touched.
- Tag `v0.7.92` will be pre-created at the cleanup commit SHA
  (`13f7084...`) and moved to the merge commit per the CC#42
  fixpoint-cascade workaround (m9-83 handoff).
- Cross-checks satisfied (at apply-time):
  - `bash scripts/check_vault_drift.sh`: CC#46 + CC#53 clean.
  - `python3 scripts/clean_m9_90_stale_branches.py --dry-run`: 0
    candidates (idempotent — nothing left to delete).
  - `cargo fmt --all -- --check`: clean.
  - `cargo clippy --workspace --all-targets -- -D warnings`: clean.
  - Recovery log: `scripts/branches-deleted-m9-90.log`.
