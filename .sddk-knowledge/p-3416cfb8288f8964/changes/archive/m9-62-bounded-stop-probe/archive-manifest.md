# Archive Manifest — m9-62-bounded-stop-probe

## Summary

m9-62 closes the m9-61 follow-up: bounded join restored in `NativeProbeBackend::stop_probe`. Single B-direct commit landed as b98b2a4 on feat/m9-62-bounded-stop-probe. Pattern matches the original HIGH-5 design (async spawn + mpsc + recv_timeout(10s)) but combined with stop-then-drain semantics from m9-61. No API change, no trait signature change.

## Cycle

| Campo | Valor |
|---|---|
| Cycle | m9-62-bounded-stop-probe |
| Base SHA | `bedfebd90e8092f8f0716742ff0bede5e4dc7b68` |
| Head SHA | `b98b2a4f23cff82292aadc1e25c8d27b460b8cd5` |
| Path | B-direct |
| Date | 2026-09-13T09:44Z |
| Branch | `feat/m9-62-bounded-stop-probe` |
| Tag | `v0.7.64` |
| Tag peel SHA | `b98b2a4f23cff82292aadc1e25c8d27b460b8cd5` |
| Peel match | `b98b2a4f23cff82292aadc1e25c8d27b460b8cd5` |
| Status | CLOSED |

## Evidence bindings

- **`apply-checkpoint.json`**: `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-M9-61-UNBOUNDED-JOIN]`
- **`verify-findings.json`**: 1 finding (FIND-M9-61-UNBOUNDED-JOIN), verdict `pass_with_findings`
- **`verify-report.md`**: Subject, Files Inventory, Gates (T0 + T2 + T4), Cross-checks, Notes, History
- **`merge-receipt.md`**: `Base SHA | bedfebd…`, `Head SHA | b98b2a4…`
- **`release-receipt.md`**: `Remote tag | v0.7.64`, `Peel match | b98b2a4…`

## Tangential modifications

1 source file changed (20 insertions, 5 deletions):

| File | Net change |
|---|---|
| `crates/chronos-native/src/probe_backend.rs` | bounded join pattern + doc |

## Cross-checks

- C1-C51: pass
- C48 meta-check: pass (0 DRIFT lines)
- No new CC added

## Follow-ups (deferred)

- **CC for stop-then-drain caller ordering**: optional static check enforcing the call order in service layer. Deferred — the call sites are stable (only ProbeService and BrowserProbeService, both updated in m9-61). Low value for a 2-site CC.
