# Archive Manifest — release-v0-9-0

## Summary

Release of `v0.9.0`. The headline of this cycle is a production defect, not a
feature: concurrent capture sessions could wedge silently, reporting `running`
forever with an empty execution log and no API able to tell. Two independent
causes were found and both fixed — a tracer that stole its own tracee, and a
test that could not fail because it read a literal the product itself produced.

The cycle also had to be restarted. `release-v020` was created bound to
`feat/release-v020` while the work had been committed straight to `main`, and
the local release route requires a cycle to point at trunk. SDDK exposes no CLI
to retarget a cycle's branch, so a fresh B-direct cycle on `main` carried the
same verification evidence and superseded it.

## Identification

| Field | Value |
|---|---|
| Cycle | `release-v0-9-0` |
| Supersedes | `release-v020` (bound to the wrong branch) |
| Date | 2026-10-04 |
| Path | B-direct |
| Base tag | `v0.1.4` (`98c4cd23`) |
| Head SHA | `aac0965909935cfa0294fd3fcf389ea33a371f6a` |
| Tag | `v0.9.0` |
| Tag object | `9debf57174ad31ef20f17b6ac71e89c084322bbd` |
| Tag peel | `aac0965909935cfa0294fd3fcf389ea33a371f6a` |
| Range | `v0.1.4..aac09659`, 360 commits, 216 files |
| Verdict | `PASS_WITH_WARNINGS` |

## Chain of custody

`release-receipt` -> `archive-manifest`, both bound to `aac09659`.

## Root cause fixed

`PtraceTracer::wait_event` called `waitpid(-1, __WALL)`, which reaps the exit
status of any child of the *process* rather than of the calling thread. The
server starts one tracer thread per session and concurrent sessions are a
product feature, so two tracer threads competed for the same children and the
loser never saw its tracee again. Fixed with `__WALL | __WNOTHREAD`, keeping
`__WALL` for syscall and clone/fork accounting. The silently-wedging resume
failure was promoted from `debug!` to `warn!`.

The premise that kept the defect alive is documented and corrected:
`crates/chronos-native/src/test_support.rs` justified `waitpid(-1)` by a single
`active_session` in production, which `multi_session.rs` disproves.

## Evidence bindings

- `crates/chronos-native/src/ptrace_tracer.rs` — `__WNOTHREAD` on `wait_event`.
- `crates/chronos-native/src/probe_backend.rs` — resume failure logged at `warn!`.
- `chronos-sandbox/tests/rec_c2_2_uat_c2.rs` — `control_capture_sees_an_event`
  and `verdict_is_unobservable`; the verdict is earned from an independent
  capture, not inherited from `server.rs:2794`.
- CI run `37228104451`: `uat_c2_01_probe_drain_is_not_an_authority ... ok` at
  `20:14:34.810Z`, 18.16s after its sibling diagnostic. A skip would have burned
  the 300s deadline, so this is a real pass.

## Verification authority, stated precisely

| Level | Where | Result |
|---|---|---|
| Integration matrix (TIER 2) | remote CI Test job | success, `Skip args:` empty, 0 deferred |
| Workflows on this SHA | GitHub Actions | 6/6 success, 0 red |
| Local canonical gate | `pipelinek` run `510619aa` | 12/12 stages, **TIER 1 only** |
| Targeted suites | local | 114 + 7 + 11 + 4 + 2 + 6 tests green |

The local gate is explicitly not integration evidence: its
`test-workspace-integration` stage echoed `TIER 2 NO EJECUTADO: CHRONOS_FULL_GATE != 1`
and ran no integration test. An earlier draft of the verification report had
described that run as "including test-workspace-integration", which would have
reproduced exactly the overstatement this project already recorded as one of its
five incidents. It was corrected before the report was stored.

## Release mechanics, recorded rather than smoothed over

The first `sddk release apply` failed with `git push ... timed out` after
creating the local tag. The tag was verified annotated and correctly aimed,
then published directly with the repository's pre-push gate active and green.
One retry was refused by that gate with `clippy FALLO` and no lint diagnostic:
another project was releasing against the same shared `CARGO_TARGET_DIR`, and
the identical command passed once the contention cleared. No `--no-verify` and
no `SKIP_PRE_PUSH_GATE` was used. The second `sddk release apply` converged with
`applied: 0`.

## Cross-checks

- `python3 scripts/regen_manifest_index_shas.py --check`: clean.
- `sddk ledger verify`: valid, 497 events, hash chain intact.
- `bash scripts/check_vault_drift.sh`: PASS.
- `sddk vault index`: rebuilt.

## Open at archive

- `DEBT-PROBE-LIVENESS-01`, product half: `probe_drain` still returns the
  hardcoded literal `"running"` at `crates/chronos-mcp/src/server.rs:2794`. The
  fix removes one cause of a silent session; the API still cannot report a dead
  worker, so a consumer would again need a control capture to find out.
  Surfacing real worker liveness is the closing condition.
- `uat_c2_01`'s three `panic!` paths do not tear down: `Drop` cannot await
  `probe_stop`, so the test needs a restructure.

## Index SHAs

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/release-v0-9-0/archive-manifest.md` | `0000000000000000000000000000000000000000000000000000000000000000` |
| implementation-receipt | `cycle-artifacts/p-3416cfb8288f8964/release-v0-9-0/implementation-receipt.json` | `59556122eeb7e6ddd0b5d0dcca9f2593797ca5c6066918493b5e68efbd56d302` |
| verification-report | `cycle-artifacts/p-3416cfb8288f8964/release-v0-9-0/verification-report.json` | `606854c47edbc8eed436c2cb074d899c7c8c19ec4e62df0e8e92d7bf9cee7920` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/release-v0-9-0/verify-findings.json` | `c5ad0b4575b14a1bb5174b1ae00321a73d30b8ea7dea04f0a30c2aa299e7b7bd` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/release-v0-9-0/release-receipt.md` | `bd43d639e58256fce281c708322d492d74393768e58ce0811cb07171913d9689` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/release-v0-9-0/merge-receipt.md` | `942a208404d44a8f9018def3345fcd190e048e5867d7c5797e70883c16f85daf` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/release-v0-9-0/release-report.md` | `cd77d917a9c194ad1785283059153bc45a3dc0cf59101eb189c74a0b562202f8` |
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/release-v0-9-0/apply-checkpoint.json` | `e7293dde21b05157d05282403c5f9eb5808316931f2b686a742f2ee07d08b98d` |
