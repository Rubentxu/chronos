# Merge Receipt — m9-80-property-policy-ownership (pending release)

**Note**: This merge-receipt is a placeholder. The cycle is at the **verify**
phase (PASSED). The actual merge to `main` and tag publication happen in the
subsequent release phase, after which this file is updated with the real merge
SHA, merge time, and remote tag. The expected pattern (per m9-79 and the SDDK
release receipt flow) is:

| Field | Value (to be filled at release) |
|---|---|
| Merge commit | TBD (predicted = `ccf8811` + merge commit) |
| Merge type | `--no-ff` (preserved cycle branch topology) |
| Source branch | `feat/m9-80-property-policy-ownership` |
| Target branch | `main` |
| Fast-forwarded? | no |
| Merged at | TBD (expected 2026-09-14T08:55Z, post-verify) |
| Pushed to origin | TBD |
| Remote tag | TBD (expected `v0.7.82`, patch bump per tag convention) |

## Commits introduced (planned)

```
90c7d4c  m9-80: define PropertyHypothesisOutcome family in domain (T0)
ae6a7df  m9-80: add T0 progress note (domain outcome types landed)
a12e7d6  m9-80: move T0 progress note from cycle-artifacts/ to changes/
d1e6a52  vault: add m9-80 row + CC#51 exception + CC#6 last-closed fix + CC#4 cascade
57c1a25  docs(handoff): append m9-80 T0 complete + 3 vault fixes + T1 next-session procedure
7990db9  m9-80: move eval_invariant into chronos_domain::property (T1)
c6c7efe  docs(handoff): append m9-80 T1 complete + repeatable pattern for T2-T5
933670d  m9-80: move eval_existence into chronos_domain::property (T2)
26a5cf4  feat(m9-80): move eval_call_path from services to domain (T3)
3d93766  m9-80: rename observe_property_target_domain + re-export Property (T4)
ccf8811  m9-80: make observe_property_target public + final verification (T5)
```

All 10 already on `feat/m9-80-property-policy-ownership` ahead of merge; main
will pick them up unchanged.

## Identity note

All 10 commits authored under `rubentxu <rubentxu@users.noreply.github.com>`
(cycle identity); no rebase performed. The merge commit will be authored by
`Chronos Maintainer <maintainer@chronos-rs.local>` (the configured repo
identity for merges) per local convention.

## No-regression note (pre-merge confirmation)

The diff has been verified end-to-end (see verify-report.md). The cycle
introduces only the diff already covered by the verify report: 4 code files
(domain owns the policy, services wraps via From) and the standard vault/
handoff bookkeeping. No prior cycle's tests or docs are touched (only the
CC#4 cascade regenerates 10 archive-manifest.md index SHA rows — mechanical).

The cycle does NOT touch: any MCP schema, any `session_*` action signature,
any JSON envelope shape. The wire shape is preserved through the From impls.
