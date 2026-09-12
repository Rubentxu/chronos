# Verify Report — m9-17-verify-findings-and-markdown-sha-drift

## Subject

`v0.7.15` on `fix/m9-17-verify-findings-and-markdown-sha-drift` (peel matches `134dc7525312275c447db8f5996740ff7c102a02`).

## Verification gates

### R1 (PASS): Drift is fixed and procedure self-validates

Re-ran all 10 cross-checks of `vault-drift-sweep.md` against HEAD post-merge:

```
CHECK 1 (ID uniqueness): empty
CHECK 2 (findings_closed ↔ Terminated): clean (m8-04-R4 + m8-07-R2 are pre-reorg expected)
CHECK 3 (apply-checkpoint ↔ tag consistency — TIGHTENED): empty
CHECK 4 (SHA-256 archive-manifest Artifact index): empty
CHECK 5 (cycles/index.md metadata consistency): OK: 33 == 33
CHECK 6 (terms/index.md "Last archive" ↔ cycles/index.md most-recent): OK: m9-17 == m9-17
CHECK 7 (change-entry Head/Base SHA ↔ apply-checkpoint): empty
CHECK 8 (apply-checkpoint + archive-manifest SHA fields exist in repo): empty
CHECK 9 (archive-manifest Head SHA + cross-references 40-char): empty
CHECK 10 (cycle artifact SHA fields — NEW): empty (verify-findings, release-receipt, merge-receipt, change-entry all consistent)
```

Cross-check #10 (the new one) returns empty on the post-fix state. Pre-fix state would have returned:

- `DRIFT: m9-11-.../verify-findings.json: subject_sha is 40 chars` — actually wait, m9-11's pre-fix subject_sha was `cd0115f8c93bddcae06e5a57f4e7e91d3a4fbb33` which IS 40 chars. C10 would have passed for length but FAILED for value mismatch (subject_sha != apply-checkpoint.head_sha).
- `DRIFT: m9-12-.../verify-findings.json: subject_sha is 7 chars` — yes, 7 < 40.
- `DRIFT: m9-13-.../verify-findings.json: subject_sha is 7 chars` — yes, 7 < 40.
- `DRIFT: m9-11-.../release-receipt.md: Head SHA is 7 chars` — yes, `cd0115f` is 7 chars.
- `DRIFT: m9-11-.../merge-receipt.md: Head SHA is 7 chars` — yes.
- `DRIFT: m9-14-.../merge-receipt.md: Head SHA is 7 chars` — yes, `3869906` is 7 chars.
- `DRIFT: m9-14-.../release-receipt.md: Head SHA is 7 chars` — yes, `3869906` is 7 chars.
- `DRIFT: m9-11-.../change-entry.md: Head SHA is 7 chars` — yes.
- `DRIFT: m9-12-.../change-entry.md: Head SHA is 7 chars` — yes.
- `DRIFT: m9-13-.../change-entry.md: Head SHA is 7 chars` — yes.

So C10 would have caught all 10 drift sites. Post-fix, all 10 are corrected.

### R2 (PASS): T0 lint clean

```
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
```

Output: 0 warnings across 14 crates.

### R3 (PASS): Schema-v1 accepted-by-design

m9-03 and m9-04 verify-findings.json use `sddk.verify-finding/v1` schema with nested `subject.head` / `subject.head_sha` field. The values are:
- m9-03: subject.head = `570d2150336fb4b159c9f9c8cc1165a7731a9e2e` (the fix commit), apply-checkpoint head = `2c98ce9a1df65d44ae865376fee46eb0d95ac425` (the docs commit peeled by v0.7.1)
- m9-04: subject.head_sha = `379759ed3d5dcc46e84c9b0bc153bab23e360b61` (an intermediate commit), apply-checkpoint head = `d6b3b8c51c50d2ce6d0fe4f8c804cf793137f0dc` (the docs commit peeled by v0.7.2)

This is **accepted-by-design drift** under the pre-m9-11 docs-peel convention. m9-11+ uses fix-peel. Migrating these would require a dedicated cycle that handles the docs-peel → fix-peel transition (rebuilding apply-checkpoint.json, re-tagging, etc.), which is out of scope for a hygiene cycle.

## Lens summary

| Lens | Findings | Evidence gaps |
|---|---|---|
| `spec-compliance` | 0 | none |
| `test-quality` | n/a | n/a |
| `vault-hygiene` | 0 | m9-11/12/13/14 SHA drift fixed, m9-17 self-validating procedure extended |

## Verdict

**PASS**

Reason tied to mandatory gates:
- `subject_identity`: PASS — clean tree, HEAD pinned at `134dc7525312275c447db8f5996740ff7c102a02`.
- `behavioral_compliance`: PASS — full SHAs in all cycle artifacts; check #10 added.
- `real_implementation`: PASS — no stubs or mocks.
- `documentation_discipline`: PASS — drift history documented; schema-v1 exception explained.
- `regression_and_build`: PASS — T0 clean; no production code touched.
- `production_readiness`: PASS — doc-only change.
- `design_and_solid`: PASS — new check enumerates all file types that store SHAs.
- `task_completeness`: PASS — all deliverables shipped together.

The cycle is ready to advance to `sddk-archive` for vault sync.
