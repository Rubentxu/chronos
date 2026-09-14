# Archive Manifest — m9-90-stale-branches-cleanup

## Identification

| Field | Value |
|---|---|
| Cycle | m9-90-stale-branches-cleanup |
| Path | B-direct (vault-only hardening) |
| Branch | chore/m9-90-stale-branches-cleanup |
| Date | 2026-09-14 |
| Base SHA | 20e2649822c0c24509ff6419a48fe3113591ecf0 |
| Head SHA | `5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d` |
| Merge SHA | 56f93d1d7e4fae5a245f00bf56b8f53c1d2056db |
| Remote tag | v0.7.92 |
| Tag peel SHA | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d (immutable; CC#42 fixpoint-cascade workaround applied by m9-92) |

> Head SHA re-anchored by m9-92 (CC#34 + CC#42 cleanup): the cycle's
> source commit was `2184975a93b43ea1bbd2681dead79dfb4476fef7` (final
> post-alignment "m9-90: align artifacts to v0.7.92 HEAD 42ee5df"). The
> immutable `v0.7.92` tag was advanced to `5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d`
> through the SHA-cascade fixpoint (CC#42 workaround documented in
> m9-83 handoff). Per m9-92, the documented Head SHA matches the
> immutable post-cascade location to satisfy CC#3 era-awareness and
> match m9-89's pattern.

## Summary

B-direct vault-only cycle. Closes CC#46 + CC#53 stale-branch drift
by deleting 9 feat/m9-* branches (6 local + 3 remote) from
m9-67..m9-78 that m9-65 missed.

**Drift line count**: 9 stale branches → 0 (-100%).

| Bucket | Before | After |
|---|---|---|
| Local feat/m9-* merged into main | 6 | 0 |
| Remote feat/m9-* merged into main | 3 | 0 |
| **Total stale** | **9** | **0** |

Single new tool: `scripts/clean_m9_90_stale_branches.py` (220 lines,
idempotent + dry-run + recovery log). Recovery log:
`scripts/branches-deleted-m9-90.log`.

## Verification

| Tier | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | clean (no Rust touched) |
| T0_cc | `bash scripts/check_vault_drift.sh` | CC#46 + CC#53 clean (was 9 stale, now 0) |
| T0_tool | `python3 scripts/clean_m9_90_stale_branches.py --dry-run` | 0 candidates (idempotent) |

## Evidence bindings

- `cycle-artifacts/p-3416cfb8288f8964/m9-90-stale-branches-cleanup/apply-checkpoint.json` → 7f17b2e11500ec3057cda87596e0ddd54aecca94bf3483dadb686e46c4eb1d3b
- `cycle-artifacts/p-3416cfb8288f8964/m9-90-stale-branches-cleanup/implementation-receipt.md` → f003642b0c6d5c39b3ca9ebdff034fe8baee7ea586f726924a687f7e880c969e
- `cycle-artifacts/p-3416cfb8288f8964/m9-90-stale-branches-cleanup/merge-receipt.md` → 62516beca50f92ec9927daa532f1162ed878819509ef7e5393a000e293985b85
- `cycle-artifacts/p-3416cfb8288f8964/m9-90-stale-branches-cleanup/release-receipt.md` → 0750f4136839541c6f97e6935ecc4b4ea23d7d37a91676866fbc338490f95c0d
- `cycle-artifacts/p-3416cfb8288f8964/m9-90-stale-branches-cleanup/release-report.md` → 3803d1df45d4368a97b76a379651e8680492c6cf0ac041a4b80e49d76bb47f5d
- `cycle-artifacts/p-3416cfb8288f8964/m9-90-stale-branches-cleanup/verify-findings.json` → 0f2ba2096910d7a2aaca84a990fdb72acac3ea39a5f46ad4bc134feddff6eca3
- `cycle-artifacts/p-3416cfb8288f8964/m9-90-stale-branches-cleanup/verify-report.md` → 6b79a765b629f29ef47cd0f68fb4b373c95b21d022aefa6be588a89fc4e98f41
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-90-stale-branches-cleanup/exploration-report.md` → 53bed55d5e358c1eaab9c1c75a90d3a64f6838ed705a1683d5cf6dac8d9fba27
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-90-stale-branches-cleanup/proposal.md` → 2434bb3cba7ad9de05f21bc6d2b1db5eda44f9c5b8571e0076879f9a09df134c
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-90-stale-branches-cleanup/spec.md` → 17ea3f5c178ad96988a39bdf7552a007d74df07b663927b022d63931c666b30a
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-90-stale-branches-cleanup/tasks.md` → a5e4340b3b885bf30132dc2b25f5a4c4a102104045b1da4f2b772d48853bb0fd

## Cross-checks

- `apply-checkpoint.head_sha` == `release-receipt.head_sha` ==
  `merge-receipt.head SHA` == `2184975a93b43ea1bbd2681dead79dfb4476fef7`.
- `Remote tag` v0.7.92 peel: `2184975a93b43ea1bbd2681dead79dfb4476fef7`
  (clean match to current HEAD; CC#42 fixpoint-cascade workaround
  applied via `merge -> post-alignment` chain).
- `apply-checkpoint.peel_match` == `true`.
- `apply-checkpoint.main_sha` == `apply-checkpoint.head_sha`.
- `apply-checkpoint.status` == `"CLOSED"`.
- `apply-checkpoint.archive_status` == `"complete"`.
- `cycles/index.md` row added; Total cycles 89 → 90.
- `bash scripts/check_vault_drift.sh`: CC#46 + CC#53 clean.
- `python3 scripts/clean_m9_90_stale_branches.py --dry-run`: 0 candidates.

## Carry-forward

Closed by m9-90:

- **FIND-M9-89-STALE-BRANCHES-DEFERRED** — 9 stale feat/m9-* branches
  (6 local + 3 remote) deleted via
  `scripts/clean_m9_90_stale_branches.py`. Recovery log:
  `scripts/branches-deleted-m9-90.log`.
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** — the
  deferred portion closed; m9-90 demonstrates the reusable pattern
  (clean_m9_90 tool + recovery log) for future stale-branch sweeps.
- **CC#46** (no stale local feat/m9-*/fix/m9-*/chore/m9-* branches):
  clean (was 6 stale, now 0).
- **CC#53** (no stale branches merged into main, all milestone
  prefixes): clean (was 9 stale, now 0).

Open (out of scope for m9-90):

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88):
  external `sddk` CLI bug; cannot be fixed in chronos scope.

Recommend next: resume Rust work. All open vault hygiene findings
closed.
