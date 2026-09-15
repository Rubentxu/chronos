# Verify Report — m9-63

**Cycle**: m9-63-stop-drain-cc
**Path**: B-direct

## Summary

Single-commit B-direct cycle that closes the second m9-61 follow-up: adds CC#52 to `vault-drift-sweep.md`, a static cross-check for the stop-then-drain ordering contract at service-layer call sites. One-file change (74 lines). No source code changes.

## Subject

| Base | Head (final) | Dirty diff digest | CWD | Verified at |
|---|---|---|---|---|
| `52ee9a2` | `8dc1063d1a8f24bd2f59fac501b848003eec63d5` | `sha256:d16e6646b10d71284c9225da9d89819528613dd56d88f4e77661f878654d6bb4` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-13T09:54:00Z |

## Files Inventory

1 file changed, 74 insertions(+), 0 deletions(-):

| File | Change |
|---|---|
| `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` | CC#52 appended: stop-then-drain ordering cross-check |

## Gates

| Gate | Status | Evidence |
|---|---|---|
| T0: cargo fmt --check | PASS | no output |
| T0: cargo clippy --workspace --all-targets -- -D warnings | PASS | no warnings (6.41s) |
| CC#52: stop-then-drain ordering on current code | PASS | 0 drift lines |
| CC#48: meta-check (every CC clean) | PASS | 52 CCs all clean |
| CC#52 self-test (synthetic violation injection) | PASS | CC#52 catches the violation |

## Cross-checks

- CC#1..CC#51: unchanged, all pass
- **CC#52 (new)**: pass on current code; self-tested with a synthetic `fn synthetic_stop` containing drain-before-stop — CC#52 emits a DRIFT line pointing at file:line

## Notes

- CC#52 uses a regex+brace-walk heuristic, not a full Rust parser. This is sufficient for the 2 known call sites (`ProbeService::stop`, `BrowserProbeService::stop`) because both have a single `fn stop(...)` body per file. If new service files are added, the `call_sites` list in CC#52 must be extended (resolution step 2).
- The brace-walking approach handles nested fns (closures, nested impls) by tracking depth — verified that the synthetic violation (a top-level `fn synthetic_stop` injected at file end) is caught.

## History

m9-63 was a planned follow-up to m9-61, recorded in the m9-61 handoff:
> "**CC enforcement**: add a static check that `ProbeBackend::stop_probe` callers do not invoke `drain_raw_events` before `stop_probe` returns."

m9-63 closes that follow-up by adding CC#52. The race itself remains closed (m9-61 + m9-62). CC#52 is forward-looking defense: any future refactor or new consumer is caught at drift-sweep time.

## Findings

None — clean state. (m10-legacy-migration)
