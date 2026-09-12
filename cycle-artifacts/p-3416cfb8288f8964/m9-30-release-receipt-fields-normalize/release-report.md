# Release Report — m9-30

## Cycle

m9-30-release-receipt-fields-normalize (v0.7.28)

## Changes

Vault metadata only:

1. **25 release-receipt.md files** (m9-03 through m9-27) normalized
   to the canonical format with required SHA fields:
   - `Head SHA` (40-char SHA matching apply-checkpoint.head_sha)
   - `Remote tag` (e.g. `v0.7.17` for m9-19)
   - `Remote tag_peel` (40-char SHA matching apply-checkpoint.remote_tag_peel)
   - `Peel match` (true)
   - `Date` (released_at timestamp)

2. **`.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`**
   - Added cross-check #22 that explicitly checks both field
     presence AND SHA consistency in release-receipt.md.
   - Updated "When to escalate" with check #22.
   - Updated Reference list with m9-30 entry.

3. **`.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`**
   - Added m9-30 row.
   - Bumped Total cycles from 45 to 46.

4. **`.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`**
   - Bumped Last archive to m9-30.
   - Bumped Last updated to 2026-09-12T12:30:00Z.

## Verification

- All 22 cross-checks: PASS
- 25 release-receipt.md files each verified to have the canonical
  SHA fields, with SHAs matching apply-checkpoint.json

## Risk

None. Vault metadata only; no code or runtime behavior affected.
