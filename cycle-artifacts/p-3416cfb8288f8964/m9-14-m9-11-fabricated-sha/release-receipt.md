# Release Receipt — m9-14-m9-11-fabricated-sha

| Campo | Valor |
|---|---|
| Cycle ID | `m9-14-m9-11-fabricated-sha` |
| Path | B-direct |
| Head SHA | `38699061891b76f90ef316914d3ba15d6eb53f83` |
| Tag | `v0.7.12` (annotated, peel matches published SHA) |
| Remote tag_peel | `38699061891b76f90ef316914d3ba15d6eb53f83` |
| peel_match | true |
| Status | RELEASED |

## Release contents

- `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/apply-checkpoint.json`:
  - `head_sha`: `cd0115f8c93bddcae06e5a57f4e7e91d3a4fbb33` → `cd0115fd8f942058cde109c72a975cab7ea7473c`
  - `remote_tag_peel`: same correction
  - `peel_match`: now genuinely true (was a lie)
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`:
  - m9-11 row `Published SHA`: `cd0115f8c93bddcae06e5a57f4e7e91d3a4fbb33` → `cd0115fd8f942058cde109c72a975cab7ea7473c`
  - Added m9-14 row
  - `Last updated` updated to 2026-09-12T10:01:00Z
  - `Total cycles`: 29 → 30
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`:
  - `Last archive`: m9-13 → m9-14
  - `Last updated` updated to 2026-09-12T10:02:00Z
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`:
  - Added cross-check #8 (SHA existence in repo)
  - Added m9-14 to the procedure reference list
  - Updated "When to escalate" wording to cover check 8

## Runtime status

No production code changed. Doc-only cycle. No T1/T2/T4-smoke gates
required.

## Tag convention

Per chronos convention: tag peeled to the **fix commit** (`3869906`),
keeping runtime semantics on the tagged SHA.

## Findings

No findings introduced or closed by this cycle that affect runtime. One
documentation drift (`m9-11 apply-checkpoint fabricated SHA`) is fixed.

Two adjacent drift classes were discovered while writing cross-check #8
but are deferred to dedicated cycles (out of m9-14 scope):

- **C2 gap (14 terms)**: `findings_closed` / `findings_introduced.no_action`
  across cycles contain 14 IDs that were never added to the
  Terminated section of `terms/index.md` (the section is empty). This
  was a pre-existing gap, not introduced by m9-14.
- **C3 short-SHA**: m9-12 and m9-13 stored 7-char SHAs in
  `remote_tag_peel` instead of full 40-char SHAs. C3 caught this
  before m9-14's fix; C8 did not (git accepts short SHAs when
  unambiguous). To be fixed in dedicated cycles.

These are documented as `findings_remaining_m9_plus` in the
apply-checkpoint.
