# m9-25: Merge Receipt

| Field | Value |
|---|---|
| Cycle | m9-25-missing-verify-findings |
| Path | B-direct |
| Base SHA | `429f01c` (main @ start) |
| Branch | `fix/m9-25-missing-verify-findings` |
| Merge target | `main` |
| Merged at | 2026-09-12T11:27:00Z |

## Changes merged

1. 6 prior cycles (m9-05..m9-10): synthesized minimal verify-findings.json files.
2. `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`: cross-check #18 added.
3. `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: m9-25 row added, total cycles 40→41.
4. `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: Last archive updated.

## Synthesis

The synthesized verify-findings.json files have:
- `cycle_id`: matching the folder name
- `subject.head` + `subject.head_sha`: from apply-checkpoint.json head_sha
- `findings`: empty array
- `_note`: explains the synthesis

The original verify-findings for these cycles was lost during the
vault reorg (or never committed). The synthesis is conservative —
it does not fabricate findings that may have existed.
