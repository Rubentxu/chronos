# Archive Manifest — m9-15-m9-12-m9-13-short-sha

| Campo | Valor |
|---|---|
| Cycle | `m9-15-m9-12-m9-13-short-sha` |
| Archived at | 2026-09-12T10:11:30Z |
| Tag | `v0.7.13` |
| Head SHA | `2441f6f3c679555dc4106ea2e8a422ed407a26a0` |
| Base SHA | `d55cdc874edd5863f5d8a65a42396b99d090b6dc` |
| Release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-15-m9-12-m9-13-short-sha/release-receipt.md` |

## Evidence bindings

Vault metadata cycles only (or vault + minimal code). Each cycle artifact bound to its SHA-256:

- `cycle-artifacts/p-3416cfb8288f8964/m9-15-m9-12-m9-13-short-sha/apply-checkpoint.json` → `6ee0325abae8a3f8dc6e9bd91bca7cb5a625a9f23c95cebd88f7d24a26f7aeee`
- `cycle-artifacts/p-3416cfb8288f8964/m9-15-m9-12-m9-13-short-sha/merge-receipt.md` → `90ebe1b72fd1d3eebd0af73fa560752057844b7782738a5bbe642fd9b324ba52`
- `cycle-artifacts/p-3416cfb8288f8964/m9-15-m9-12-m9-13-short-sha/release-receipt.md` → `5e0875437d3cc897394105420796b122235bdc5ca1b789b3401db09336f3cb93`
- `cycle-artifacts/p-3416cfb8288f8964/m9-15-m9-12-m9-13-short-sha/release-report.md` → `3d6bd36edcfd8360adab62b21fd0cf60653e2377fbf7a1e205606f2022322ff6`
- `cycle-artifacts/p-3416cfb8288f8964/m9-15-m9-12-m9-13-short-sha/verify-findings.json` → `cf4ccb5cce05ecc1d1f6370c0d6a28854a8ea8f76789e6877a8d1e47caeeb302`
- `cycle-artifacts/p-3416cfb8288f8964/m9-15-m9-12-m9-13-short-sha/verify-report.md` → `e28d71d019f6ce1843161a3b9e8532107d83d581433060e4b4448bd577885d71`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-15-m9-12-m9-13-short-sha/change-entry.md` → `f3b8c52e79349f0c75319559fc0fa909112604f7ff6b2b8e4042b02949ac5cf1`

## Artifact index (SHA-256)

| File | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-15-m9-12-m9-13-short-sha/merge-receipt.md` | `90ebe1b72fd1d3eebd0af73fa560752057844b7782738a5bbe642fd9b324ba52` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-15-m9-12-m9-13-short-sha/release-receipt.md` | `5e0875437d3cc897394105420796b122235bdc5ca1b789b3401db09336f3cb93` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-15-m9-12-m9-13-short-sha/release-report.md` | `3d6bd36edcfd8360adab62b21fd0cf60653e2377fbf7a1e205606f2022322ff6` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-15-m9-12-m9-13-short-sha/verify-report.md` | `e28d71d019f6ce1843161a3b9e8532107d83d581433060e4b4448bd577885d71` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-15-m9-12-m9-13-short-sha/apply-checkpoint.json` | `421affab74ce7235aa53c87111b35ea887fbf110ca2b959c06fec0bb249322c5` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-15-m9-12-m9-13-short-sha/verify-findings.json` | `47a2869df36262beedc5b3330fb49881eccca746ae9e0bfb608464f539fa1e55` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-15-m9-12-m9-13-short-sha/change-entry.md` | `f3b8c52e79349f0c75319559fc0fa909112604f7ff6b2b8e4042b02949ac5cf1` |

## Notes

m9-15 closes a format inconsistency in m9-12 and m9-13 cycle artifacts
(head_sha and remote_tag_peel were 7-char short SHAs while sibling
fields used 40-char). Tightens cross-check #3 to require
`len(remote_tag_peel) == 40` so future cycles cannot regress.
