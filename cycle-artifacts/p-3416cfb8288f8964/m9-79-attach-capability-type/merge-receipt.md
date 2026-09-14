# Merge Receipt — m9-79-attach-capability-type

| Field | Value |
|---|---|
| Branch | feat/m9-79-attach-capability-type |
| Date | 2026-09-14 |
| Base SHA | 009b75037357d069775ad4d7a0661684e27fc9ca |
| Head SHA | f41abd4580d078a3f5f1255ffd543573a4fa702d |
| Merge commit | `f41abd4580d078a3f5f1255ffd543573a4fa702d` |
| Merge type | `--no-ff` (preserved cycle branch topology) |
| Source branch | `feat/m9-79-attach-capability-type` |
| Target branch | `main` |
| Fast-forwarded? | no |
| Merged at | 2026-09-14T06:27:45+02:00 |
| Pushed to origin | yes (`009b750..f41abd4 main -> main`) |
## Commits introduced

```
917fcb0  feat(m9-79): distinguish ptrace attach capabilities
eba71a1  docs(handoff): persist m9 attach and SDDK recovery state
```

Both already on `feat/m9-79-attach-capability-type` ahead of merge; main picked them
up unchanged.

## Post-conditions

- `git rev-parse HEAD`              = `f41abd4580d078a3f5f1255ffd543573a4fa702d`
- `git rev-parse origin/main`       = `f41abd4580d078a3f5f1255ffd543573a4fa702d`
- `git status --porcelain` (post-merge, on `main`) = empty
- Working tree clean.

## No-regression note

The merge introduced only the diff already covered by the verify-report: one
literal in `crates/chronos-services/src/session_lifecycle.rs`, one matching
assertion in the sandbox, two manual updates, and the handoff docs commit. No
prior cycle's tests or docs are touched.

## Identity note

Pre-existing commits authored under `rubentxu <rubentxu@users.noreply.github.com>`;
no rebase performed. The merge commit is authored by `Chronos Maintainer
<maintainer@chronos-rs.local>` (the configured repo identity for merges) per
local convention.