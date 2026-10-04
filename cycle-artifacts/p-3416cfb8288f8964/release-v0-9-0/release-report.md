# release-report — v0.9.0

- cycle: `p-3416cfb8288f8964/release-v0-9-0` (B-direct)
- main SHA: `aac0965909935cfa0294fd3fcf389ea33a371f6a`
- tag: `v0.9.0` (annotated, peels to the same SHA)
- runtime status at write time: RELEASE_PENDING, phase release

## What this release contains

The headline is a production defect, not a feature. Concurrent capture
sessions could wedge silently: the session reported `running` forever while the
execution log stayed empty, and no API could tell. Two things had to be true for
that to happen and both were fixed.

**The tracer stole its own tracee.** `PtraceTracer::wait_event` used
`waitpid(-1, __WALL)`, which reaps the exit status of any child of the
*process*, not of the calling thread. The server runs one tracer thread per
session, and concurrent sessions are a product feature, not an accident. Two
tracer threads competed for the same children and the loser never saw its tracee
again. Fixed with `__WALL | __WNOTHREAD`, keeping `__WALL` for syscall and
clone/fork accounting.

**The test that should have caught it could not.** `probe_drain` answers
`"status": "running"` as a hardcoded literal, and the empty state was byte for
byte identical whether the producer never started, died, parked or was simply
idle. The test consumed that literal, so its "the environment is at fault"
verdict was circular. `uat_c2_01` now earns its verdict from an independent
control capture on the same host, and two guards that could not fail were
removed.

Along the way: a hypothesis that measurement refuted and the change was reverted
instead of shipped; a false premise in `test_support.rs` documented and corrected;
a silently-wedging failure promoted from `debug!` to `warn!`; CC#4 manifest
SHA drift closed across 102 manifests.

## Verification authority

| Level | Where | Result |
|---|---|---|
| Integration matrix (TIER 2) | remote CI Test job, run `37228104451` | success, `Skip args:` empty, 0 tests deferred |
| Other workflows on this SHA | GitHub Actions | 6/6 success, 0 red |
| Local canonical gate | `pipelinek` run `510619aa` | 12/12 stages, **TIER 1 only** |
| Targeted suites | local | chronos-native 114/114, multi_session 7/7, program_scenarios 11/11, race_depth 4/4, m1_04 2/2, rec_c2_2_uat_c2 6/6 |
| Lint/format/manifests | local + pre-push | fmt clean, clippy `-D warnings` clean, CC#4 clean |

The local gate is explicitly TIER 1: its integration stage echoed
`TIER 2 NO EJECUTADO: CHRONOS_FULL_GATE != 1` and ran no integration test. Its
stage-level success is not counted as integration evidence. The integration
matrix ran in CI, which executes the same cargo invocation.

## Honest limits

The tracer race is scheduler-dependent and was not reproduced on demand, so
there is no controlled before/after measurement of it. That it happens in
practice rests on two independent reproductions of the same signature: the
independent verifier at `first_event_after_ms=300047`, and CI on `684b3d16` with
the control capture seeing an event at 5 ms.

`DEBT-PROBE-LIVENESS-01` stays open on the product side. The fix removes one
cause of a silent session; the API still cannot report a dead worker, so a
consumer would again need a control capture to find out. Surfacing real worker
liveness is the closing condition.

## Release mechanics note

The first `sddk release apply` failed with `git push ... timed out` after
creating the local tag. The tag was verified annotated and correctly aimed, then
published directly with the repository's own pre-push gate active and green. One
retry was refused by that gate with `clippy FALLO` and no diagnostic; the cause
was another project releasing against the same shared `CARGO_TARGET_DIR`, and
the identical command passed once contention cleared. No bypass was used. The
second `sddk release apply` converged with `applied: 0`.
