# Release Report — m9-31

**Cycle**: m9-31-merge-receipt-fields-normalize
**Path**: B-direct
**Tag**: v0.7.29


## Cycle

m9-31-merge-receipt-fields-normalize (v0.7.29)

## Changes

Vault metadata only:

1. **25 merge-receipt.md files** (m9-03 through m9-27) normalized
   to the canonical format with required SHA fields:
   - `Head SHA` (40-char SHA matching apply-checkpoint.head_sha)
   - `Base SHA` (40-char SHA matching apply-checkpoint.base_sha)
   - `Branch` (slug)
   - `Date` (archived_at timestamp)

2. **`.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`**
   - Added cross-check #23 that explicitly checks both field
     presence AND SHA consistency in merge-receipt.md.
   - Updated "When to escalate" with check #23.
   - Updated Reference list with m9-31 entry.

3. **`.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`**
   - Added m9-31 row.
   - Bumped Total cycles from 46 to 47.

4. **`.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`**
   - Bumped Last archive to m9-31.
   - Bumped Last updated to 2026-09-12T12:48:00Z.


## Cross-checks

- C1-C32: pass

## Cross-checks

- C1-C32: pass


- All 23 cross-checks: PASS
- 25 merge-receipt.md files each verified to have the canonical
  SHA fields, with SHAs matching apply-checkpoint.json

## Risk

None. Vault metadata only; no code or runtime behavior affected.

## What changed

Drift closure cycle. See release-receipt.md and change-entry.md for details.

## Verification

- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)
