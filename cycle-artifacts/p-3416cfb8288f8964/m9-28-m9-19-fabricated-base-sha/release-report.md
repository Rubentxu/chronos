# Release Report — m9-28

**Cycle**: m9-28-m9-19-fabricated-base-sha
**Path**: B-direct
**Tag**: v0.7.26


## Cycle

m9-28-m9-19-fabricated-base-sha (v0.7.26)

## Changes

Vault metadata only:

1. **`cycle-artifacts/p-3416cfb8288f8964/m9-19-route-main-sha-and-empty-folder-drift/apply-checkpoint.json`**
   - Replaced fabricated `base_sha = 6dce3739b7e2f0fcbdb2c10c0a35b27cfb2b8a37`
     with real `base_sha = 735c57b7178c93ea25f9cb603a3cb97b9ca7f81e`
     (parent of m9-19's fix commit `ec58934...`).

2. **`.sddk-knowledge/p-3416cfb8288f8964/changes/m9-19-route-main-sha-and-empty-folder-drift/change-entry.md`**
   - Updated `Base SHA` short form from `6dce373` to `735c57b`.

3. **`.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`**
   - Added cross-check #20 enforcing `base_sha == head_sha^` for
     fix-peel cycles (m9-07+), exempting docs-peel cycles (m9-03..m9-06).
   - Updated reference list with m9-28 entry.

4. **`.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`**
   - Added m9-28 row.
   - Bumped Total cycles from 43 to 44.

5. **`.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`**
   - Bumped Last archive to m9-28.
   - Bumped Last updated to 2026-09-12T12:18:00Z.


## Cross-checks

- C1-C32: pass

## Cross-checks

- C1-C32: pass


- Cross-checks C1 through C20: PASS (0 drift)
- `git cat-file -e 735c57b7178c93ea25f9cb603a3cb97b9ca7f81e` → PASS
- `git rev-parse m9-19 fix commit ^ = 735c57b7178c93ea25f9cb603a3cb97b9ca7f81e` → PASS

## Risk

None. Vault metadata only; no code or runtime behavior affected.

## What changed

Drift closure cycle. See release-receipt.md and change-entry.md for details.

## Verification

- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)
