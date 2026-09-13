# Change: m9-61 probe stop-then-drain race fix

## Summary

Closed a drain/stop race in the probe lifecycle. Drain could be invoked while the probe thread was still emitting events, losing them. Fix: reorder stop-then-drain in ProbeService / BrowserProbeService, make NativeProbeBackend::stop_probe blocking, update trait doc.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-61-ms-race-fix` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `97ce507d209eb1bb5d61a4d5b41c0f8bc068343a` |
| Head SHA | `5d4c00d36b14e56349b615e0a1d65186b273c510` |
| Tag | `v0.7.63` |

## Subject

- base_sha: `97ce507d209eb1bb5d61a4d5b41c0f8bc068343a`
- head_sha: `5d4c00d36b14e56349b615e0a1d65186b273c510`
- cycle: m9-61
- branch: `feat/ms-race-fix`
- date: 2026-09-13
- tag: `v0.7.63`
- findings_closed: 1 (FIND-MS-RACE-FIX-DRAIN-STOP)
- findings_introduced.no_action: 0

## Files changed

- (modified) `crates/chronos-domain/src/adapter.rs` — ProbeBackend::stop_probe doc updated to reflect blocking semantics + ADR-0005 cite
- (modified) `crates/chronos-native/src/probe_backend.rs` — NativeProbeBackend::stop_probe blocking rewrite (handle.join() inline)
- (modified) `crates/chronos-services/src/probe.rs` — ProbeService::stop reordered to stop-then-drain
- (modified) `crates/chronos-services/src/browser_probe.rs` — BrowserProbeService::stop reordered to stop-then-drain
- (modified) `crates/chronos-browser/src/adapter.rs` — BrowserAdapter::stop_probe: drop event_buffer.clear() (drain now happens after stop)
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-61-ms-race-fix/apply-checkpoint.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-61-ms-race-fix/verify-findings.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-61-ms-race-fix/verify-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-61-ms-race-fix/merge-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-61-ms-race-fix/release-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-61-ms-race-fix/release-report.md`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-61-ms-race-fix/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-61-ms-race-fix/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-61 row added)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last archive bumped)
- (modified) `.gitignore` (ignore .jcode/ and docs/*.zip)

## Cross-checks

CC#1..CC#51: pass. No drift introduced. No new CC added — the race is closed by construction (blocking stop_probe + stop-then-drain contract), not by retroactive detection.
