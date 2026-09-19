# Release receipt — rec-c3-3-services-inversion (C3.3.0 recon)

## Released artifacts

- Branch: `main`
- Cycle merge base: `188f2e182df180f9f947ccb1f6ad87ca028216e6` (REC-C3.2 close, already on origin/main)
- Cycle merge head: `8cd653fb8db853c0a2f8eed2c7928c1621b70da6` (cycle commit)
- Tag: **none** — recon-only cycle. No behavior change; only documentation. Workspace version `0.1.1` is preserved from `v0.1.1` (peels to `33b4f790`). No patch bump is justified because no production code changed.
- Project: `p-3416cfb8288f8964`
- Cycle: `p-3416cfb8288f8964/rec-c3-3-services-inversion`
- Release type: `none (recon-only)`
- Predecessor tag: `v0.1.1` (preserved)
- `workspace.package.version`: `0.1.1` → `0.1.1` (unchanged)

## Tags verified

| Tag | Peeled SHA | Annotated | Notes |
|---|---|---|---|
| `v0.1.1` | `33b4f79004b7634a9e88b7919f8b42e854c420e8` | yes | inherited from REC-C3.1; not modified |

Remote verification:

```
$ git ls-remote origin v0.1.1
960e5d56f1f4553f6746947b4e1f118d5517f3fc    refs/tags/v0.1.1
```

Local tag verify (annotated):

```
$ git show-ref refs/tags/v0.1.1
960e5d56f1f4553f6746947b4e1f118d5517f3fc    refs/tags/v0.1.1
$ git ls-remote origin refs/tags/v0.1.1
960e5d56f1f4553f6746947b4e1f118d5517f3fc    refs/tags/v0.1.1
```

The annotated tag at `960e5d56...` peels to `33b4f790` (REC-C3.1 head).
The local and remote SHA of the tag match.

## Gate receipts

| Gate | Receipt | Status |
|---|---|---|
| `implementation-complete` | `gate-implementation-complete-42abec9b28420ba2-1` | passed |
| `tests-pass` | `gate-tests-pass-dfa67c5d8570dac3-1` | passed |
| `policy-compliant` | `gate-policy-compliant-dfa67c5d8570dac3-1` | passed |
| `debt-severity-assigned` | `gate-debt-severity-assigned-dfa67c5d8570dac3-1` | passed |
| `debt-priority-assigned` | `gate-debt-priority-assigned-dfa67c5d8570dac3-1` | passed |
| `no-pending-effects` | (per-release direct local merge + no tag; see below) | passed |
| `release-uat-approved` | (waived — recon-only, no behavior change, no MCP surface change) | waived |

`no-pending-effects` was satisfied by the absence of CI/CD and forge
adapters and by the direct local commit + no-tag sequence (the previous
`v0.1.1` tag peels unchanged to `33b4f790`).

## Cycle path

A-min (recon + dependency map; sub-cycle scope: C3.3.0 only).

## Findings

None. Recon-only cycles do not introduce findings; the carry-over
findings `C31-DEBT-01/02/03` (from REC-C3.1) are re-observed but
**not closed** in this cycle (closes when C3.3.1..3 land the
identity/storage seam and the inversion). See
`cycle-artifacts/p-3416cfb8288f8964/rec-c3-3-services-inversion/verification-report.md`
§ "Carry-forward findings (re-observed, NOT closed)" for the exact
re-observation.

## Verification highlights

- V1 `check_hex_boundary.py`: 0 errors, 0 notes.
- V2 `check_architecture_contracts.py --strict-legacy`: PASSED.
- V3 `cargo check -p chronos-services --all-targets`: 0 errors / 0 warnings.
- V4 `cargo fmt --all -- --check`: clean.
- 4 acceptance criteria (from specification.md): all met.
- No source code changes; only 2 new files in `cycle-artifacts/`.

## Notes

- REC-C3.3 is the third sub-cycle of REC-C3 (Hexagonal boundary closure). This C3.3.0 deliverable is recon + dependency map only. C3.3.1..5 are separate cycles.
- The `v0.1.1` workspace version is preserved; no version bump because no production code changed.
- No public API change in any crate. No ratchet movement (legacy-evb baseline still 0). Architecture baseline unchanged (5 transitional edges in `reconstruction-contracts.toml[architecture.known_dependency_violations]`).

## Canonical SHA fields (added by CIH-C.1)

| Field | Value |
|---|---|
| Cycle | `rec-c3-3-services-inversion` |
| Head SHA | `188f2e182df180f9f947ccb1f6ad87ca028216e6` |
| Remote tag | (no remote tag pushed; archived in branch) |
| Remote tag_peel | `188f2e182df180f9f947ccb1f6ad87ca028216e6` |
| Peel match | true |
| Date | (original release date not verifiable from current artifacts; see _restoration_note) |

## Restoration note

CIH-C.1 (2026-09-19) appended these canonical SHA fields because the
original release-receipt.md artifact produced by the prior cycle did
not carry them. The Head SHA and remote_tag_peel were reconstructed
from the apply-checkpoint.json `head_sha` field (which in turn was
reconciled from git history by CIH-C.1 — see apply-checkpoint.json restorations for related cycles
similar reconciliation entries). The "Remote tag" entry reflects that
no remote tag was pushed for this cycle (it was archived in branch).
