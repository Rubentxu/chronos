# Merge receipt — rec-c3-3-services-inversion (C3.3.0 recon)

## Merge outcome

- Branch: `main`
- Strategy: single-trunk (no `--no-ff` because C3.3.0 produces exactly one commit on top of the post-rec-c3.2 close base).
- Pre-merge base: `188f2e182df180f9f947ccb1f6ad87ca028216e6` (REC-C3.2 close, already pushed to origin/main).
- Merge head: `8cd653fb8db853c0a2f8eed2c7928c1621b70da6` (cycle commit).
- Diff digest: `2f0fa6f9...` (computed by `git diff 188f2e18..8cd653fb`).

```
$ git log --oneline 188f2e18..8cd653fb
8cd653fb docs(rec-c3.3): C3.3.0 recon + dependency map artifacts
```

```
$ git diff --stat 188f2e18..8cd653fb
 .../rec-c3-3-services-inversion/exploration-report.md | 238 +++++++
 .../rec-c3-3-services-inversion/specification.md      | 136 ++++
 2 files changed, 374 insertions(+)
```

Recon-only cycle. No file outside `cycle-artifacts/` was touched. No
production Rust code modified. No Cargo.toml modified. No tag created
(recon does not justify a version bump — same as REC-C3.2).

## Cycle path

A-min (recon-only). Sub-cycle scope: C3.3.0 only.

## Hard-gate pre-conditions

- HEAD before merge == `origin/main`: confirmed (`188f2e18`).
- Working tree clean before cycle: confirmed.
- No `--no-ff` (single commit, no PR).
- Tag `v0.1.1` peels unchanged to `33b4f790` (C3.1 head); no tag
  created by this cycle.

## Notes

- REC-C3.3 is the third sub-cycle of REC-C3 (Hexagonal boundary closure). This C3.3.0 deliverable is recon + dependency map only. C3.3.1..5 are separate cycles (each one an ADR or a code sub-cycle), gated by the operator.
- The architectural rule the cycle documents (services → ports, never services → concrete adapter) is the closure criterion for HEX-002 in C3.3.5.
- ADR for the composition-root placement (option 3: `chronos-services::composition`) lands in C3.3.1.
- Carry-forward findings (C31-DEBT-01/02/03) are re-observed, not closed.

## Canonical SHA fields (added by CIH-C.1)

| Field | Value |
|---|---|
| Head SHA | `188f2e182df180f9f947ccb1f6ad87ca028216e6` |
| Base SHA | `188f2e182df180f9f947ccb1f6ad87ca028216e6` |
| Branch | `rec-c3-3-services-inversion` |
| Date | (original merge date not verifiable from current artifacts; see _restoration_note) |

## Restoration note

CIH-C.1 (2026-09-19) appended these canonical SHA fields because the
original merge-receipt.md artifact produced by the prior cycle did
not carry them. The Head SHA and Base SHA were reconstructed from
the apply-checkpoint.json `head_sha` and `base_sha` fields (which in
turn were reconciled from git history by CIH-C.1 — see
apply-checkpoint.json restorations for related cycles). The Branch
field reflects the cycle's branch (from apply-checkpoint.json `branch` field).
The Date field is left as a restoration placeholder because the
original receipt did not record a verifiable merge timestamp.
