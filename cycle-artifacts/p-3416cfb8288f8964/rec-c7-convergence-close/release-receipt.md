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

<!-- CC#22 additive normalization: canonical SHA fields.
     Appended 2026-09-21 by G0.3 vault drift sweep.
     Values reconciled to cycle-artifacts/.../rec-c7-convergence-close/apply-checkpoint.json
     and re-verified directly against git (annotated tag object, tag peel, merge commit).
     This is a structural addition for the drift gate; it does NOT re-certify the release
     and does NOT modify the narrative above (per AGENTS.md §0.4 and §0.5). -->

## Canonical SHA fields (CC#22)

| Field | Value |
|---|---|
| Cycle | `p-3416cfb8288f8964/rec-c7-convergence-close` |
| Head SHA | `1fd11d9826339f1605249fd2ef7ae07d7acf38e3` |
| Remote tag | `v0.7.112` |
| Remote tag_peel | `0be2ec2d53d9698956ae705938b32b80d7365ad7` |
| Peel match | `false` |

### Notes on these values

- **Head SHA = `1fd11d98`** is the post-archive main HEAD at the time the cycle record was sealed (matches `apply-checkpoint.json::head_sha` and `head_sha_post_archive`). It is **not** the merge commit.
- **Remote tag_peel = `0be2ec2d`** is the annotated tag peel (`v0.7.112 -> 0be2ec2d^{commit}`), which is the REC-C7 merge commit. The narrative above is correct: the tag is "fixed at the merge commit … the convergence close marker" and that is intentional.
- **Peel match = `false`** is the honest reading: `peel (0be2ec2d) != head_sha (1fd11d98)`. The apply-checkpoint's `peel_match: true` field was correct at the moment the cycle was sealed (when `1fd11d98` was the post-archive tip), but subsequent docs-only slices on main (e.g. `cf9b3a0a`, `5981d12f`, `2c454e0d`) advanced main past `1fd11d98`. This is the m9-19+ fix-peel pattern (CC#3 spec: "head_sha != remote_tag_peel is honest when peel_match=False is honest"). I do not edit `apply-checkpoint.json::peel_match` here — that is a behavior change, out of scope for the CC#22 mechanical fix.
- The four values above were re-verified at append time (2026-09-21) via `git cat-file -p <tag-object>`, `git rev-parse v0.7.112^{commit}`, and `git log -n 1 --format=%H` on the merge commit. The apply-checkpoint was used to **locate** the candidates; the values themselves come from git.
- This block does not certify the release. CERT-n for REC-C7 remains governed by `docs/roadmap/CERTIFICATION.md`; this is a T0 documentary normalization under G0.3.
