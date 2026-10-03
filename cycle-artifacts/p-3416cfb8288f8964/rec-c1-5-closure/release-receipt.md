# Release Receipt — rec-c1-5-closure

| Field | Value |
|---|---|
| Head SHA | `a1a79c80122d3e8a859f44bb29f043f5422e1324` |
| Base SHA | `17367f2d5ebcd4510de43ba2eef6c8b61cedd6de` |
| Branch | `feat/rec-c1.5-closure` |
| Date | `2026-09-20` |
| Tag | `rec-c1.5-closure` |
| Remote tag | `rec-c1.5-closure` |
| Remote tag_peel | `5bbf774888c3bced7967e3e060859d9a78d7035a` |
| Peel match | true |
| Cycle | `p-3416cfb8288f8964/rec-c1-5-closure` |
| Path | `A-lite` |
| Route | `local` |
| Version | `0.1.4` |

- tag_object: 79193338994679e3e7ebb66fc0ff6a568348b295
- tag_peel: 5bbf774888c3bced7967e3e060859d9a78d7035a
- merge_commit: a1a79c80122d3e8a859f44bb29f043f5422e1324
- base: efb4498953feabb1f0a0bb1c841ac03e93d46f1c
- workspace_version_at_closure: 0.1.4
- released_at: 2026-09-20 (cycle work); re-verified 2026-09-30

## Provenance of the canonical SHA fields (added 2026-10-03)

This receipt previously carried `Tag`, and the peel only in the `tag_peel`
bullet above. It had no `Remote tag`, `Remote tag_peel` or `Peel match`
rows, so vault-drift check **CC#22** reported three drift lines against this
cycle — one of the two cycles keeping the sweep red since `b5edae57`.

The three rows are the canonical format that CC#22's own resolution procedure
prescribes. Their values are not copied from `apply-checkpoint.json`: that
file's `remote_tag_peel` was **wrong**, and copying it would have closed the
drift by writing a false SHA into the receipt.

`apply-checkpoint.json` recorded `remote_tag_peel: a1a79c80…`, which is the
**merge commit**, not the tag peel. Verified against the live remote on
2026-10-03:

```
$ git ls-remote --tags origin "rec-c1.5-closure*"
79193338994679e3e7ebb66fc0ff6a568348b295	refs/tags/rec-c1.5-closure
5bbf774888c3bced7967e3e060859d9a78d7035a	refs/tags/rec-c1.5-closure^{}
```

`refs/tags/…^{}` is the peel, so the tag resolves to `5bbf7748…` — the value
this receipt's own "Integration verification" section already recorded on
2026-09-30. The local tag object agrees: `79193338…` peels to `5bbf7748…`.
`a1a79c80…` is a real commit and an ancestor of `HEAD`, it is simply not what
the tag points at.

`Peel match: true` is kept because the tag exists on the remote and peels to a
commit that is an ancestor of `main`.

## What was delivered

REC-C1.5 closed five findings, all verified closed in
`verify-findings.json`:

- `FIND-REC-C1.5-EXEC-LOG-ROOT-MUST-BE-CANONICAL` — `ExecutionLog` root
  resolved once, env-overridable, backed by `OnceLock`.
- `FIND-REC-C1.5-MCP-STARTUP-NEEDS-BOOTSTRAP` — `ChronosServer::try_new`
  calls `bootstrap_execution_logs`; a failure surfaces as the typed
  `ChronosServerInitError::ExecutionLogBootstrap`.
- `FIND-REC-C1.5-DELETION-MUST-BE-DURABLE` — deleting a session removes its
  durable logs, and they stay gone across restart.
- `FIND-REC-C1.5-CLEAN-STOP-MUST-SEAL` — a clean stop seals the log tail
  (`tail_sealed=true`) and a restart bootstraps it.
- `FIND-REC-C1.5-READINESS-INVARIANT` — registry length is observable through
  a public accessor, regression-verified by commenting out the bootstrap call
  (registry reports `0` against an expected `2`).

## Verification evidence

- `verify-findings.json`: `verdict: PASS`, `all_passed: true`, 5/5 findings
  `closed`, tiers `T0`, `T1`, `T3`, `T4-smoke`.
- Real-process UATs: R1 unclean restart, R2 identical-stale cursor, R3 sealed
  persistence, plus the UAT-R4 delete durability invariant.
- `tests-pass` and `policy-compliant` gate receipts exist for both
  `phase.verify.complete.a-min` and `phase.verify.complete.a-lite`.
- `implementation-complete` gate receipt exists for `phase.build.complete`.

## Integration verification, 2026-09-30

Re-observed on trunk at `6effe2047816f856c963f7fbe9f4488508c3079e`:

- `git merge-base --is-ancestor a1a79c80 HEAD` → exit `0`.
- `git merge-base --is-ancestor 5bbf7748 HEAD` → exit `0`.
- `git ls-remote --tags origin rec-c1.5-closure` → present, peel
  `5bbf7748`.
- Working tree clean at verification time.

## Scope boundary

This cycle closes the REC-C1.5 findings only. It does not bump the workspace
version and does not create a semver release: the trunk version is `0.1.4`,
published by `release-pipeline-honesty-v014` under tag `v0.1.4`. The tag
`rec-c1.5-closure` is a **cycle tag**, and the VERSION LOCKSTEP rule refuses
to plan a release for it because it is not a semver tag. That refusal is
correct behaviour and is recorded rather than worked around: this cycle's work
is already integrated and tagged on the trunk, and it needs no further release
action.
