# Change: m9-62 bounded stop probe HIGH-5 timeout

## Summary

Restored the HIGH-5 bounded-join timeout in `NativeProbeBackend::stop_probe`. m9-61's inline `handle.join()` had lost the 10s upper bound. Fix: spawn waiter + mpsc::channel + recv_timeout on caller's stack; abandon on timeout.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-62-bounded-stop-probe` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `bedfebd90e8092f8f0716742ff0bede5e4dc7b68` |
| Head SHA | `b98b2a4f23cff82292aadc1e25c8d27b460b8cd5` |
| Tag | `v0.7.64` |

## Subject

- base_sha: `bedfebd90e8092f8f0716742ff0bede5e4dc7b68`
- head_sha: `b98b2a4f23cff82292aadc1e25c8d27b460b8cd5`
- cycle: m9-62
- branch: `feat/m9-62-bounded-stop-probe`
- date: 2026-09-13
- tag: `v0.7.64`
- findings_closed: 1 (FIND-M9-61-UNBOUNDED-JOIN)
- findings_introduced.no_action: 0

## Files changed

- (modified) `crates/chronos-native/src/probe_backend.rs` — bounded join pattern restored in stop_probe; doc updated
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-62-bounded-stop-probe/apply-checkpoint.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-62-bounded-stop-probe/verify-findings.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-62-bounded-stop-probe/verify-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-62-bounded-stop-probe/merge-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-62-bounded-stop-probe/release-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-62-bounded-stop-probe/release-report.md`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-62-bounded-stop-probe/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-62-bounded-stop-probe/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-62 row added)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last archive bumped)

## Cross-checks

CC#1..CC#51: pass. No drift introduced. No new CC added.
