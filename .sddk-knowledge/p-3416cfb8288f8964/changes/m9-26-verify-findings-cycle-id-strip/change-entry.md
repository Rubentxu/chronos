# Change: m9-26 verify-findings.json cycle_id prefix strip

| Field | Value |
|---|---|
| Cycle | m9-26-verify-findings-cycle-id-strip |
| Base SHA | `38e0e3a` |
| Head SHA | `a7d43e0` |
| Tag | `v0.7.24` (peels to `a7d43e0`) |
| Path | B-direct |
| Date | 2026-09-12 |
| Author | jcode (auto-mode B-direct) |

## Summary

m9-23 stripped workspace prefix from apply-checkpoint.json cycle_id
but missed verify-findings.json. m9-04 verify-findings.json had
'p-3416cfb8288f8964/m9-04-side-table-key-layout'.

m9-26 strips the prefix and extends cross-check #16.

## Cross-check

Cross-check #16 extended to cover verify-findings.json.

## Verification

- T0 gate: PASS
- All 18 cross-checks: PASS
