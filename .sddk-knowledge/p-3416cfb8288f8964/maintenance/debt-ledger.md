# Pre-existing debt ledger (from REC-C4 verify, 2026-09-20)

Debt items found during REC-C4 verification that reproduce on `origin/main`
(75d04447) and are therefore NOT regressions of this cycle. Recorded per rule
B5 (no waivers — pre-existing failures get debt entries).

| ID | Source | Description | Repro |
|---|---|---|---|
| DEBT-C4-01 | vault drift CC#11 | `rec-c3.3-train-b` suspended cycle stuck in `apply_complete_pending_capture_session`, expected `CLOSED`. Suspended artifact dirs (`cycle-artifacts/_suspended-rec-c3.3-train-b/`) are untracked leftovers. | `bash scripts/check_vault_drift.sh` |
| DEBT-C4-02 | vault drift CC#18 | Missing `verify-findings.json` for cycles rec-c3-hexagonal-closure, rec-c3.3-train-b, rec-c3.3.4-native, rec-c3.5-residual-inversion, rec-c4-solid-connascence | same |
| DEBT-C4-03 | vault drift CC#22 | `rec-c3.3-train-b/release-receipt.md` missing Head SHA / Remote tag / tag_peel / Peel match fields | same |
| DEBT-C4-04 | vault drift CC#56 | `chronos-sandbox/src/client/identity.rs:151-164` mutates process env (`std::env::set_var`/`remove_var` for `CHRONOS_MCP_EXPECTED_SHA`). Test-only helper; pre-exists on main. | same |

These do not gate REC-C4 (verified pre-existing on main). Candidate follow-up
cycle: `m-rec-c4.5-vault-debt` to close DEBT-C4-01..04.
