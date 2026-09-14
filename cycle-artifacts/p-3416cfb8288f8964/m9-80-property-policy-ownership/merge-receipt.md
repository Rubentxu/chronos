# Merge Receipt — m9-80-property-policy-ownership

| Field | Value |
|---|---|
| Merge commit | `7874e5c8e972172c024b07f60746f0e06df92f9d` |
| Merge type | `--no-ff` (preserved cycle branch topology) |
| Source branch | `feat/m9-80-property-policy-ownership` |
| Target branch | `main` |
| Fast-forwarded? | no |
| Merged at | 2026-09-14T08:49Z |
| Pushed to origin | yes (`82e219f..7874e5c main -> main`) |

## Commits introduced

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
34b67b2  m9-80: add cycle-artifacts for verify phase (T5)
8012342  docs(handoff): append m9-80 T3+T4+T5+verify-artifacts landed; release phase pending
```

12 commits since base; all already on `feat/m9-80-property-policy-ownership`
ahead of merge; main picked them up unchanged.

## Post-conditions

- `git rev-parse HEAD`              = `7874e5c8e972172c024b07f60746f0e06df92f9d`
- `git rev-parse origin/main`       = `7874e5c8e972172c024b07f60746f0e06df92f9d`
- `git tag -l "v0.7.82"`            = `v0.7.82`
- `git rev-parse v0.7.82^{commit}`  = `7874e5c8e972172c024b07f60746f0e06df92f9d`
- `git status --porcelain` (post-merge, on `main`) = empty
- Working tree clean.

## No-regression note

The merge introduced only the diff already covered by the verify report: 4
code files (domain owns the policy, services wraps via From) plus the 12
cycle/cycle-artifacts bookkeeping files (verify-findings, verify-report,
implementation-receipt, merge-receipt, release-report, release-receipt,
apply-checkpoint, T0 progress note, cycles/index.md row, handoff appends,
archive-manifest.md CC#4 cascade). No prior cycle's tests or docs are touched
(only the CC#4 cascade regenerates 10 archive-manifest.md index SHA rows —
mechanical).

The cycle does NOT touch: any MCP schema, any `session_*` action signature,
any JSON envelope shape. The wire shape is preserved through the From impls.

## Identity note

All 12 commits authored under `rubentxu <rubentxu@users.noreply.github.com>`
(cycle identity); no rebase performed. The merge commit is authored by
`Chronos Maintainer <maintainer@chronos-rs.local>` (the configured repo
identity for merges) per local convention.
