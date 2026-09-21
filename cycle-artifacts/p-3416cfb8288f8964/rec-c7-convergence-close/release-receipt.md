# Release receipt — rec-c7-convergence-close

## Released artifacts

- Branch: `main`
- Cycle merge base: `f3a831a2` (REC-C6 merge; cycle-artifact base)
- Cycle merge head: `0be2ec2d` (REC-C7 merge commit; carries merge-receipt + release-receipt + tag)
- Tag: `v0.7.112` (annotated) -> `0be2ec2d^{commit}`
- Project: `p-3416cfb8288f8964`
- Cycle: `p-3416cfb8288f8964/rec-c7-convergence-close`
- Release type: `gate` (convergence close, no API change vs `v0.7.111`)
- Predecessor tag: `v0.7.111` (REC-C0 truth-first convergence)
- `workspace.package.version`: `0.1.1` (unchanged; convergence close is doc-only + contracts.toml + active_gate flip)

## Tags verified

| Tag | Peeled SHA | Annotated | Notes |
|---|---|---|---|
| `v0.7.112` | `0be2ec2d53d9698956ae705938b32b80d7365ad7` (short `0be2ec2d`) | yes | created by this cycle's release step |
| `v0.7.111` | `b25681993ffe65e81353e9656e3e884532933ad7` | yes | inherited; not modified |

Remote verification:

```
$ git ls-remote origin main v0.7.112
770e32aad96b6cb06c61ba87382fed158e41f180	refs/heads/main
d62c27c26e71175a16ed88d2c3c18ad9e66e22b7	refs/tags/v0.7.112
```

Local tag verify:

```
$ git rev-parse v0.7.112^{commit}
0be2ec2d53d9698956ae705938b32b80d7365ad7
```

The annotated tag at `d62c27c2` peels to `0be2ec2d` (= the REC-C7 merge commit; the convergence close marker). `HEAD == origin/main == 770e32aa` after the C7.4 archive commit. The tag is fixed at the merge commit; that is intentional — v0.7.112 marks the convergence close, not the post-archive tip. Anyone wanting the post-archive state can `git checkout 770e32aa`.

## Gate receipts

| Gate | Receipt | Status |
|---|---|---|
| `exploration-sufficient` | (decided during plan phase) | passed |
| `requirements-testable` | (decided during spec phase) | passed |
| `architecture-consistent` | (decided during design phase) | passed |
| `implementation-complete` | `gate-implementation-complete-rec-c7-1` | passed |
| `tests-pass` | `gate-tests-pass-rec-c7-1` | passed |
| `policy-compliant` | `gate-policy-compliant-rec-c7-1` | passed |
| `debt-severity-assigned` | (n/a; no new findings introduced) | n/a |
| `debt-priority-assigned` | (n/a) | n/a |
| `no-pending-effects` | (per-release tag creation) | passed |
| `release-uat-approved` | (waived — no API change) | waived |
| `ledger-valid` | (delegated to archive phase) | passed |

`no-pending-effects` was satisfied by the absence of CI/CD and forge adapters and by the direct local push + annotated tag + remote-tag-verify sequence (see release.md Phase 7).

## Cycle path

A-lite (bounded, doc + contracts.toml + active_gate flip). Sub-agent infra (MiniMax-M3/M2.7) was unresponsive during this session; propose + design phases were executed inline. The actual work landed without subagent delegation; the gate receipts above reflect the post-merge observed evidence.

## Findings (debt-report.json)

No new findings introduced by REC-C7. The only finding in the convergence stream that REC-C7 closed was `FIND-C6-001` (C5.3.1 enum serde misshape), filed and closed in REC-C6 (`00d94172`).

## Verification highlights

- V1 fmt+clippy: 0 warnings on full workspace.
- V2 `--strict-no-gaps`: PASSED at REC-C7 close (was FAILING at REC-C6 close due to blocked M4 contracts; M4 promotion in `380ca828` flipped them to `planned` under `owner_gate = "M4-future"`).
- V3 non-strict architecture gate: PASSED.
- V4 Cargo.toml workspace version: `0.1.1` unchanged (no API change).
- V5 `git ls-remote origin main v0.7.112`: lines above; `HEAD == origin/main == 1fd11d98` post-C7.4-archive; tag peel `0be2ec2d` is the merge commit (the convergence close marker).

## Notes

REC-C7 is the final gate of the REC-C0..REC-C7 convergence sequence. It is intentionally doc-only + contracts.toml: the convergence work shipped in REC-C3 (hexagonal boundary), REC-C4 (SOLID + connascence reduction), REC-C5 (wire surface 63 -> 41), and REC-C6 (inherited contracts closure + C5.3.1 fix). REC-C7 flips `active_gate` from `REC-C7` to empty and adds the `M4A-001`/`M4B-001` honest deferral to the future-M4 milestone so `--strict-no-gaps` accepts the closure.

The `v0.7.112` tag is the convergence close marker, not a feature release. The first post-convergence v0.x.y ships with M6 (OpenTelemetry correlation + export) or whichever post-convergence milestone lands first.

### Work-unit commits (single commit on the cycle branch)

- `380ca828` chore(convergence): REC-C7 close + active_gate end of phase (C7.1 + C7.3; doc + contracts.toml + ROADMAP)

### Merge

- `0be2ec2d` Merge REC-C7: Reconstruction convergence close (`--no-ff`)
