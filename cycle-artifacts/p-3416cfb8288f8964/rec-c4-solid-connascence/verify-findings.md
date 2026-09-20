# REC-C4 verify findings — 2026-09-20

Cycle: rec-c4-solid-connascence (branch feat/rec-c4-solid-connascence)

## Slices verified (independently by orchestrator)

- C4.1 SOLID-001 (6c2e0d36): TraceAdapter split into CaptureLifecycle + DebugInspect via blanket impls from legacy trait. Zero impl churn. Contract SOLID-001 gap → partial (4b58a936).
- C4.2 CONN-001 (e901f285, 84 files): MonotonicNs + WallClockMs newtypes in chronos-domain, #[serde(transparent)] u64 wire compat (tested), TimestampNs = alias of MonotonicNs. Contract notes updated (a56fd511), stays partial (chronos-log boundary pending).
- C4.3 CONN-002 (ff559f31, 5 files): SubscriptionId newtype typed through chronos-mcp server + chronos-services (observe/output). Stays partial (probe_id pending).

## Gates (real observed results)

| Gate | Result |
|---|---|
| cargo fmt --all -- --check | FMT_OK |
| clippy --workspace --all-targets -D warnings | clean (Finished, no errors) |
| chronos-domain --lib | 173 passed, 0 failed |
| chronos-native --lib serial | 109 passed, 0 failed (13s) |
| workspace lib (excl. sandbox/e2e/native) | 16/16 suites ok, 0 failed |
| chronos-mcp --lib --tests | 109 passed, 0 failed |
| chronos-services --lib --tests | 391 passed, 0 failed |
| T3 workspace tests (branch) | all ok, exit 0 |
| T4-smoke e2e_connectivity | 1 passed |
| T4-smoke analytics_tools (serial) | 4 passed (after binary rebuild; initial failures were a stale-binary artifact — target dir had a /tmp/origin-main-test path poisoning chronos-ebpf rebuild) |
| check_architecture_contracts.py | PASSED |
| check_hex_boundary.py | OK: hexagonal boundary clean |

## Surprises / limitations

- Vault drift sweep reports CC#11/18/22/56 drift; all verified pre-existing on origin/main (75d04447). Recorded as DEBT-C4-01..04 in maintenance/debt-ledger.md. Not a regression.
- analytics_tools first run failed 4/4 with MCP init timeout; root cause was a stale/poisoned build (dirty ebpf paths from /tmp/origin-main-test checkout sharing CARGO_TARGET_DIR). After `cargo build --bin chronos-mcp`, all 4 pass serially.
- Known ptrace flake hit again when native lib ran parallel (AGENTS.md §6.5); serial run clean.

## Verdict

PASS. All three slices deliver their contracted scope; contracts honestly partial where coverage is partial.
