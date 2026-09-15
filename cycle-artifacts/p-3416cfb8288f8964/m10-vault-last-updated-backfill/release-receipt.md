# Release Receipt — m10-vault-last-updated-backfill

**Cycle**: `p-3416cfb8288f8964/m10-vault-last-updated-backfill`
**Path**: B-direct
**Published subject (main HEAD)**: `fa92a6ee40aea67a9dbac9301a14fafd3c15b7af` (merge commit)
**Tag**: `v0.7.105` (annotated)
**Tag peel (`vN^{commit}`)**: `fa92a6ee40aea67a9dbac9301a14fafd3c15b7af` — matches main HEAD ✓
**Tag type**: annotated (`git tag -a`)
**Tag message**: `v0.7.105 — m10-vault-last-updated-backfill (B-direct: cycles/index.md + terms/index.md Last updated backfill; Total cycles = 98)`

## Remote tag (verification)

| Field | Value |
|---|---|
| Remote tag | `refs/tags/v0.7.105` |
| Remote tag_peel (`v0.7.105^{commit}`) | `fa92a6ee40aea67a9dbac9301a14fafd3c15b7af` |
| Match between local peel and remote peel? | YES — `git rev-parse v0.7.105^{commit}` matches both |
| Push argv (tag) | `git push origin v0.7.105` |
| Push argv (main) | `git push origin main` |
| Push exit codes | 0 / 0 |
| Push output (main) | `b70b78ef..fa92a6ee  main -> main` |
| Push output (tag) | `* [new tag]  v0.7.105 -> v0.7.105` |

## Diff snapshot (base → published main HEAD)

| File | Change |
|---|---|
| `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | +12/-0 (Last updated + Total cycles + m9 cycles + m10 cycles + Last archive metadata rows added) |
| `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | +13/-1 (Last updated + Last archive + Active/Backlog/Terminated counters metadata rows) |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-{02,70,71,72,73,74,75,76,79,80,81,82}-*/archive-manifest.md` | ×12 SHA-rewrites (cycles/index.md + terms/index.md SHA fixes via `regen_manifest_index_shas.py`) |

Total: 14 files, +49/-25.

## Branch hygiene

- Branch `feat/m10-vault-last-updated-backfill` retained per archive rule (do not delete before manifest is persisted).
- Local branch exists (`git branch --list feat/m10-vault-last-updated-backfill` → present).
- Remote tracking: branch was pushed to `origin/feat/m10-vault-last-updated-backfill` via `git push` during the cycle? — **No, B-direct did not push the feature branch separately**, only main + tag. This matches the prior m10 cycle pattern.

## Operational change

Default operator behaviour is **byte-identical** to v0.7.104 (m10-ms-cap-discovery-followup). The change is purely a vault-metadata backfill:
- `cycles/index.md` now carries the canonical `Total cycles | 98` field that `CC#39 Part C` parses, plus a `Last updated | <iso>` field that `CC#42 Part B` parses.
- `terms/index.md` similarly gains a `Last updated` field for `CC#42 Part C`.

No code changed, no tests changed. The 12 archive-manifest.md SHA rewrites are mechanical (CC#4 fixpoint cascade via `scripts/regen_manifest_index_shas.py`).

## Acceptance evidence

- `cargo fmt --all -- --check` — clean (untouched code).
- `cargo clippy -p chronos-mcp -p chronos-services --all-targets -- -D warnings` — clean (untouched crates).
- `cargo test -p chronos-mcp --lib --tests --no-fail-fast` — 148 tests, 0 failures (unchanged).
- `cargo test -p chronos-services --lib --tests --no-fail-fast` — 274 tests, 0 failures (unchanged).
- `python3 scripts/regen_manifest_index_shas.py --check` — clean (98 manifests verified).
- `bash scripts/check_vault_drift.sh` — only CC#18 reports drift (3 false-positive lines on `HANDOFF-*.md` files at `cycle-artifacts/p-3416cfb8288f8964/`); CC#42 Parts B+C are now clean; CC#39 Part C is now clean. Pre-existing infra noise.
- `sddk ledger verify` — event_count 206 (+1 from v0.7.104's 205; one append for `release.complete`).

## Status

RELEASED. Cycle proceeds to archive phase.
