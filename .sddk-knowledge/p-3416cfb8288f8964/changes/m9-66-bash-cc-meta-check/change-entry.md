# Change: m9-66 vault drift detection hardening

## Summary

Hardened vault drift detection by fixing two silently-broken bash CCs and adding CC#54 (bash meta-check, sibling of CC#48) to auto-validate the 7 bash CCs alongside the 46 python CCs. Discovered and regenerated 54 stale SHAs in m9-01..m9-10 archive-manifests.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-66-bash-cc-meta-check` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `c46851baf07c13d2e6d680e35b54c14bd7698b76` |
| Head SHA | `963a143aacd5e005fb39593afea060106f172c00` |
| Tag | `v0.7.68` |

## Subject

- base_sha: `c46851baf07c13d2e6d680e35b54c14bd7698b76`
- head_sha: `963a143aacd5e005fb39593afea060106f172c00`
- cycle: m9-66
- branch: `feat/m9-66-bash-cc-meta-check`
- date: 2026-09-13
- tag: `v0.7.68`
- findings_closed: 3 (FIND-M9-66-CC4-SILENT, FIND-M9-66-CC5-OFF-BY-16, FIND-M9-66-NO-BASH-META-CHECK)
- findings_introduced.no_action: 0

## Files changed

- (modified) `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` — CC#4 fix, CC#5 fix, CC#54 added
- (modified) `scripts/check_vault_drift.sh` — refactored to invoke both meta-checks
- (modified) 10 archive-manifest files in `changes/archive/m9-01..m9-10-*/archive-manifest.md` — regenerated 54 SHAs
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-66-bash-cc-meta-check/apply-checkpoint.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-66-bash-cc-meta-check/verify-findings.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-66-bash-cc-meta-check/verify-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-66-bash-cc-meta-check/merge-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-66-bash-cc-meta-check/release-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-66-bash-cc-meta-check/release-report.md`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-66-bash-cc-meta-check/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-66-bash-cc-meta-check/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-66 row added)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last archive bumped)

## Cross-checks

CC#1..CC#54: pass (no drift). CC#54 is a new bash meta-check (sibling of CC#48) that aggregates drift across the 7 bash CCs. Both CC#48 and CC#54 are invoked by `scripts/check_vault_drift.sh`.

## Follow-ups (deferred)

- **CC smoke test cycle**: the "fix a broken CC, then fix what it would have caught" pattern recurred (m9-65 stale branches from CC#46 gap; m9-66 54 stale SHAs from CC#4 broken regex). A future cycle could periodically inject drift into each CC and assert detection.
- **Sandbox test warm-up**: `test_session_start_via_v2_then_session_stop_via_v2` in probe_lifecycle fails when run alone but passes when run with the full file. Likely a binary warm-up issue (first MCP server spawn is slow). Not a regression but should be investigated.
