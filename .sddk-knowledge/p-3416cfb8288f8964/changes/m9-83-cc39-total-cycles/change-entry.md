# Change: m9-83 Total cycles field off-by-one fix

## Subject

- Closed pre-existing CC#39 drift: `Total cycles` field in `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` read `84` while the actual row count was `83`.
- Bumped the field to `83` so CC#39 (cycles/index.md literal field vs row count) is clean.
- Flipped the m9-83 row from `OPEN (T0 done; T1-T7 pending)` to `CLOSED`.
- Bumped `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` Last updated timestamp and Last archive cycle.

## Files changed

- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` — `Total cycles` field 84 → 83; m9-83 row OPEN → CLOSED.
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` — Last updated 11:00Z → 11:02Z; Last archive m9-82 → m9-83-cc39-total-cycles.
- `cycle-artifacts/p-3416cfb8288f8964/m9-83-cc39-total-cycles/` — full artifact set (6 files) created.
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-83-cc39-total-cycles/change-entry.md` — this file.
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-83-cc39-total-cycles/archive-manifest.md` — archive manifest.

## Summary

Trivial B-direct literal fix to close the CC#39 drift that had been
lingering on `main` since m9-78 (when a cycle row was added to the
index but no folder was created). Single-line edit, no code, no
schemas. The cycle's other vault files (exploration-report, proposal,
spec, tasks) already existed from T0; the cycle-artifacts and
change-entry/archive-manifest were created at T4 to satisfy the
vault-drift CCs.

## Cross-check

- `python3 -c "import re; t=open('.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md').read(); val=t.split('| Total cycles | ')[1].split(' |')[0]; rows=re.findall(r'^\| (m9-\d+) \|', t, re.MULTILINE); assert int(val)==len(rows); print('PASS')"` → `PASS`.
- `bash scripts/check_vault_drift.sh` no longer reports CC#39 in the DRIFT list (only CC#48 + CC#51 remain, both script-level pre-existing).
- All m9-83 cycle-artifacts have SHA-256 hashes captured in the archive-manifest.
- Carry-forward FIND-M9-83-CC39-TOTAL-CYCLES-OFF-BY-ONE closed.
