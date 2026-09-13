# Archive Manifest — m9-66-bash-cc-meta-check

## Summary

m9-66 hardens vault drift detection by fixing CC#4 (broken awk regex, silently never fired), CC#5 (off-by-16 regex), and adding CC#54 (bash meta-check, sibling of CC#48) so the 7 bash CCs are auto-validated alongside the 46 python CCs. Two B-direct commits landed as 963a143 on feat/m9-66-bash-cc-meta-check. Tag v0.7.68.

## Cycle

| Campo | Valor |
|---|---|
| Cycle | m9-66-bash-cc-meta-check |
| Base SHA | `c46851baf07c13d2e6d680e35b54c14bd7698b76` |
| Head SHA | `963a143aacd5e005fb39593afea060106f172c00` |
| Path | B-direct |
| Date | 2026-09-13T12:18Z |
| Branch | `feat/m9-66-bash-cc-meta-check` |
| Tag | `v0.7.68` |
| Tag peel SHA | `963a143aacd5e005fb39593afea060106f172c00` |
| Peel match | `963a143aacd5e005fb39593afea060106f172c00` |
| Status | CLOSED |

## Evidence bindings

- **`apply-checkpoint.json`**: `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-M9-66-CC4-SILENT, FIND-M9-66-CC5-OFF-BY-16, FIND-M9-66-NO-BASH-META-CHECK]`
- **`verify-findings.json`**: 3 findings, verdict `pass_with_findings`
- **`verify-report.md`**: Subject, Files Inventory, Drift Evidence (pre/post-cycle), Gates, Cross-checks, History
- **`merge-receipt.md`**: `Base SHA | c46851b…`, `Head SHA | 963a143…`
- **`release-receipt.md`**: `Remote tag | v0.7.68`, `Peel match | 963a143…` (true)

## Tangential modifications

12 files changed across two commits (257 insertions, 69 deletions):

| File | Net change |
|---|---|
| `.sddk-knowledge/.../vault-drift-sweep.md` | +~150, -5 (CC#4 fix, CC#5 fix, CC#54 added) |
| `scripts/check_vault_drift.sh` | +50, -13 (refactored: CC#48 + CC#54 invocation) |
| 10 × `archive/m9-0X-*/archive-manifest.md` | 56 lines changed (54 SHAs regenerated) |

## Cross-checks

- CC#1..CC#53: pass (no drift)
- CC#48 meta-check: pass (46 python CCs all clean)
- CC#54 (new, bash meta-check): pass (7 bash CCs all clean)
- T0 + T1 + T4-smoke: pass

## Follow-ups (deferred)

- **CC smoke test cycle**: the "fix a broken CC, then fix what it would have caught" pattern has now repeated twice (m9-65, m9-66). A future cycle could implement periodic drift injection to confirm every CC still fires.
- **Sandbox test warm-up ordering**: `test_session_start_via_v2_then_session_stop_via_v2` fails alone, passes with the rest of probe_lifecycle. Investigate binary warm-up.
