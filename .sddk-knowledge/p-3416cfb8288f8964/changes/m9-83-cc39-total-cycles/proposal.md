# Proposal: m9-83 CC#39 Total cycles off-by-one

> **Cycle**: `p-3416cfb8288f8964/m9-83-cc39-total-cycles`
> **Status**: proposal-complete
> **Date**: 2026-09-14
> **Path**: B-direct

## Intent

Close the pre-existing CC#39 part-C drift ("cycles/index.md Total
cycles | 84" but actual folder count is 82) by correcting the metadata
field to match the actual count.

## Scope

- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` line 115:
  change `| Total cycles | 84 |` → `| Total cycles | 82 |`.

## Acceptance

- `grep -n 'Total cycles' .sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`
  returns the new value (82).
- `bash scripts/check_vault_drift.sh` exits 0 (no CC drift).
- `python3 scripts/regen_manifest_index_shas.py --check` exits 0.
- No other CCs affected.

## Out of scope

- Removing any rows from cycles/index.md (none are missing — both
  sides equal 82).
- Audit of historical Total-cycles bump accuracy (out of cycle scope;
  not requested).
- M9-78 missing-folder story (m9-78 row IS in cycles/index.md and a
  folder DOES exist at `cycle-artifacts/p-3416cfb8288f8964/m9-78-safe-attach-detach`;
  the off-by-2 was purely in the metadata field, not in any actual
  cycle row).

## Carry-forward

- Closes: CC#39 (part C only; parts A and B were already passing).
- New: none.
