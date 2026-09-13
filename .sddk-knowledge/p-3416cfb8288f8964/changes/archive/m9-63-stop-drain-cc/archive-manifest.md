# Archive Manifest — m9-63-stop-drain-cc

## Summary

m9-63 closes the second m9-61 follow-up: a static cross-check (CC#52) for the stop-then-drain ordering contract at service-layer call sites. Single B-direct commit landed as 8dc1063 on feat/m9-63-stop-drain-cc. One-file change (vault-drift-sweep.md, +74 lines). No source code changes.

## Cycle

| Campo | Valor |
|---|---|
| Cycle | m9-63-stop-drain-cc |
| Base SHA | `52ee9a2f2ba6233f03e46c7e5c139721779ba8d4` |
| Head SHA | `8dc1063d1a8f24bd2f59fac501b848003eec63d5` |
| Path | B-direct |
| Date | 2026-09-13T09:54Z |
| Branch | `feat/m9-63-stop-drain-cc` |
| Tag | `v0.7.65` |
| Tag peel SHA | `8dc1063d1a8f24bd2f59fac501b848003eec63d5` |
| Peel match | `8dc1063d1a8f24bd2f59fac501b848003eec63d5` |
| Status | CLOSED |

## Evidence bindings

- **`apply-checkpoint.json`**: `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-M9-63-MISSING-STOP-DRAIN-CC]`
- **`verify-findings.json`**: 1 finding (FIND-M9-63-MISSING-STOP-DRAIN-CC), verdict `pass_with_findings`
- **`verify-report.md`**: Subject, Files Inventory, Gates (T0 + CC#52 + CC#48), Cross-checks, Notes, History
- **`merge-receipt.md`**: `Base SHA | 52ee9a2…`, `Head SHA | 8dc1063…`
- **`release-receipt.md`**: `Remote tag | v0.7.65`, `Peel match | 8dc1063…`

## Tangential modifications

1 file changed (74 insertions, 0 deletions):

| File | Net change |
|---|---|
| `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` | CC#52 appended (+74) |

## Cross-checks

- C1-C51: pass
- C52 (new): pass on current code; self-tested
- C48 meta-check: pass (52 CCs all clean)

## Follow-ups (deferred)

- **CC#52 call_sites extension**: if a new `ProbeBackend` consumer is added to `chronos-services/`, extend the list manually. Tracked implicitly — every future m9 cycle that touches service files should re-run CC#48 to confirm no drift.
