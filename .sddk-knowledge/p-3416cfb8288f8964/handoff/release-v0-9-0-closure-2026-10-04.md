# Closure note — v0.9.0 (release-v0-9-0), 2026-10-04

## What closed

`v0.9.0` published at `aac0965909935cfa0294fd3fcf389ea33a371f6a`, tag object
`9debf57174ad31ef20f17b6ac71e89c084322bbd`, peeling to the same SHA. Cycle
`release-v0-9-0` reached RELEASED and then archive.

## Two cycles that were closed as wrong, not as done

**`release-v0-2-0`** — created by mistake as A-full. The release path in this
repository is B-direct, which skips explore. Closed by supersede.

**`release-v020`** — created with `--branch feat/release-v020`, but the work had
been committed straight to `main`. The local release route refuses a cycle that
does not point at trunk, and SDDK has no CLI to retarget a cycle's branch:
`replan` only restages to `propose|specify|design|tasks|apply`. Hand-editing the
runtime state was rejected; instead a new B-direct cycle on `main` carried the
same verification evidence and superseded it. The verify work was not redone,
only re-bound: the released SHA and every piece of evidence were already
bound to `aac09659`.

## An error worth recording

While probing what `evaluate-gate` would accept, a receipt was written for real:

```
gate-tests-pass-dc6ca868269080de-1  outcome=failed  evidence={"probe":1}
```

That was not a test failure and not a real evaluation — it was a careless
probe, and the tool persisted it. It was immediately superseded by the genuine
evaluation `gate-tests-pass-dc6ca868269080de-2`, which carries the material
evidence. The failed receipt is left in place rather than deleted: the ledger
is an audit trail, and a record that quietly omits a mistake is worth less than
one that shows it. The cycle did not move on the failed receipt — it was still
`OPEN/verify` afterwards, and the transition only happened once the real
receipts existed.

## A correction made before it became a false record

The first draft of `verification-report.json` said the local canonical gate
covered "12/12 stages including test-workspace-integration". Reading the run's
events showed that stage had echoed `TIER 2 NO EJECUTADO: CHRONOS_FULL_GATE != 1`
and executed no integration test. The report was corrected and the distinction
recorded in a dedicated `t4_authority` field: the integration authority is the
remote CI Test job, which runs the same cargo invocation with an empty
`Skip args:` list.

This mattered because the same overstatement had already been recorded as one
of this project's five historical incidents, and because it was the specific
thing the independent verifier had caught. A report that repeats a known
overstatement is worse than no report.

## A gate that failed for an environmental reason

The first `sddk release apply` failed with `git push ... timed out` after
creating the local tag. The tag was verified annotated and aimed at the right
SHA, then pushed directly with the pre-push gate active. That push was refused:
`clippy FALLO`, with no lint diagnostic in the output. The identical command
re-run on its own exits 0 in 45.81s. The cause was another project
(`sddk-framework`) releasing against the same shared
`CARGO_TARGET_DIR=/var/home/rubentxu/cargo-targets`, so two clippy runs
contended for the build-directory lock.

The hook was not bypassed. It explicitly forbids `--no-verify` unless the
remote gate is known not to run the command, and the remote gate does run it.
The push was retried and passed with fmt OK, clippy OK and CC#4 clean across
102 manifests.

## Open, deliberately

- `DEBT-PROBE-LIVENESS-01`, product half. `probe_drain` still returns the
  hardcoded literal `"running"` at `crates/chronos-mcp/src/server.rs:2794`.
  `__WNOTHREAD` removes one cause of a silent session; the API still cannot
  report a dead worker.
- `uat_c2_01`'s three `panic!` paths do not tear down, because `Drop` cannot
  await `probe_stop`.

Neither is a release gate breach; both are recorded as tracked debt so the
`PASS_WITH_WARNINGS` verdict is not read as a plain pass.
