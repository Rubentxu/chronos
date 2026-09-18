# REC-C3.1 apply-progress

**Cycle:** `p-3416cfb8288f8964/rec-c3-1-application-ports`
**Path:** A-full
**Phase:** Build -> Verify
**Updated:** 2026-09-18

## Subject

- Base: `8b5dcebe2c94db1910d1ccce36467afcffe29b77` (close of rec-c2-5 handoff)
- Head: `646a5d93ae2f693c104088be8f2904e07f81acb5`
- Diff digest: `see shell-run log`
- Commits since base: 8 (one artifact commit + 7 apply commits).

### Commit list

```
646a5d93 feat(scripts): add check_hex_boundary.py and update HEX-001 ledger
c5b95cc4 test(domain): add behavioral tests for the ports suite
344944dd feat(domain): register session_id and wire ports/id module
f87ad239 feat(domain): add TelemetryReceiver port + noop + in-memory
a71ad51f feat(domain): add SessionRepository port + in-memory impl
5adb6223 feat(domain): add probe ports (factory/registry/controller)
7d48dc0d feat(domain): add session_id type and ports module skeleton
b6dcd77f feat(rec-c3.1): exploration + spec + design + tasks for application ports
```

## Tasks completed (B1..B10)

- B1: ports/execution_log.rs (51 lines) — placeholder port for storage backend; deferred-real-shape comment references REC-C3.3.
- B2: ports/probe.rs (169 lines) — ProbeFactory, ProbeRegistry, ProbeController + Null* impls.
- B3: ports/session.rs (163 lines) — SessionRepository, SessionHandle, SessionState, InMemorySessionRepository.
- B4: ports/telemetry.rs (180 lines) — TelemetryReceiver, NoopTelemetry, InMemoryTelemetry, Metric, Counters, TelemetryError.
- B5: ports/mod.rs wiring + lib.rs `pub mod session_id;`.
- B6: already covered by lib re-exports.
- B7: tests/{probe,session,telemetry}_ports.rs + tests/common/mod.rs (350 lines total, 19 integration tests).
- B8: scripts/check_hex_boundary.py (203 lines) — zero-dep Python checker enforcing ports/* outbound purity + surface shape.
- B9: reconstruction-contracts.toml — HEX-001 `gap -> partial` with four evidence rows (script/compile/tests/module); HEX-002 stays `gap` with C3.3 defer note.
- B10: docs/ROADMAP.md — in-progress note for REC-C3.1 added below the active milestone banner.

## Cross-cutting decisions (documented in artifact commit)

- `chronos_domain::session_id::SessionId` introduced as a thin newtype over `String` to break the cyclic dep with `chronos_log::SessionId`. This was the only way to declare session-identity-aware ports in `chronos_domain` without flipping the dependency direction. REC-C3.3 introduces the domain-side alias once services are inverted.
- `ExecutionLogProvider` is a documented placeholder module. Adding `chronos_log` as a `chronos_domain` dep would fail Cargo with a cyclic edge. See `ports/execution_log.rs` module docs for the C3.3 plan (split `NewExecutionRecord` into domain, or move `ExecutionLogBackend` wholesale).

## Mechanical checks run during apply

| Check | Command | Result | exit |
|---|---|---|---|
| 1 | `cargo fmt --all -- --check` | exit 0 | 0 |
| 2 | `cargo check -p chronos-domain --all-targets` | clean (0 errors / 0 warnings) | 0 |
| 3 | `cargo clippy -p chronos-domain --all-targets -- -D warnings` | clean | 0 |
| 4 | `cargo test -p chronos-domain --tests` | 164 passed | 0 |
| 5 | `cargo test -p chronos-domain --lib` | 145 passed | 0 |
| 6 | `python3 scripts/check_hex_boundary.py` | OK | 0 |
| 7 | `python3 scripts/check_architecture_contracts.py --strict-legacy` | PASSED | 0 |
| 8 | `cargo check --workspace --lib` | clean (24s) | 0 |

Per-request dependency: `CARGO_TARGET_DIR=$PWD/.cargo-target` because the CI target dir is the workspace-local `.cargo-target/`.

## Out-of-scope drift

- `Cargo.toml` of `chronos-domain` NOT modified (uuid was already a dep).
- `Cargo.toml` of `chronos-log` NOT modified (chronos_log's dependency on chronos_domain is preserved and unchanged).
- No new forbidden dependency edges (verified by check #7).
- No new uses of legacy tokens (`EventBus`, `snapshot_raw`, `drain_fired`, `TraceAdapter`, `UnsupportedOperation`).

## Notes for verify

- Sandbox smoke selection: e2e_connectivity + analytics_tools + program_scenarios (C3.1 touched chronos-domain only; serverside hot path untouched). See transition context for selection rationale.
- HEX-001 went to `partial`. HEX-002 stays `gap` until REC-C3.3 closes the services inversion. The ledger is honest about this: the `verified_requires = ["evidence", "uat", "verify"]` policy requires UAT to flip to `verified`, and UAT here means a live-server session that exercises one of the new ports through MCP — that is a C3.3 deliverable.
