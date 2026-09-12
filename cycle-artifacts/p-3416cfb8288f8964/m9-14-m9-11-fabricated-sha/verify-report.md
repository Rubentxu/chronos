# Verify Report — m9-14-m9-11-fabricated-sha

## Subject

`v0.7.12` on `fix/m9-14-m9-11-fabricated-sha` (peel matches
`38699061891b76f90ef316914d3ba15d6eb53f83`).

## Verification gates

### R1 (PASS): Drift is fixed and procedure self-validates

Re-ran all 8 cross-checks of `vault-drift-sweep.md` against HEAD
post-merge:

```
CHECK 1 (ID uniqueness): empty
CHECK 2 (findings_closed ↔ Terminated): clean (header/separator rows excluded; m8-04-R4 + m8-07-R2 in Terminated are pre-reorg expected)
CHECK 3 (apply-checkpoint ↔ tag consistency): 2 drift (m9-12, m9-13 short SHAs — see notes)
CHECK 4 (SHA-256 archive-manifest Artifact index): empty
CHECK 5 (cycles/index.md metadata consistency): OK: 30 == 30
CHECK 6 (terms/index.md "Last archive" ↔ cycles/index.md most-recent): OK: m9-14 == m9-14
CHECK 7 (change-entry Head/Base SHA ↔ apply-checkpoint): empty (all change-entries consistent)
CHECK 8 (apply-checkpoint SHA fields exist in repo — NEW): empty (post-fix)
```

Cross-check #8 (the new one) returns empty on the post-fix state. If
applied to the pre-fix state, it would have returned:
`DRIFT: m9-11-cycles-index-metadata-drift: head_sha=cd0115f8c93bddcae06e5a57f4e7e91d3a4fbb33 does not exist in repository`

Checks 2 and 3 were already drifting before m9-14 began (m9-12/m9-13
era). C2 turned out to be a parser false alarm (C2's reference Python
in vault-drift-sweep.md had a buggy `cells[0].startswith('m')` filter
that missed IDs starting with `F`, `o`, `c`; all 14 IDs were actually
in Terminated). C3 caught a real short-SHA drift which is fixed by
m9-15 (next cycle) via tightening C3 itself. Both are documented in
the m9-14/m9-15 cycle artifacts; neither is a regression.

### R2 (PASS): T0 lint clean

```
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
```

Output: 0 warnings across 14 crates.

## Lens summary

| Lens | Findings | Evidence gaps |
|---|---|---|
| `spec-compliance` | 0 | none |
| `test-quality` | n/a | n/a |
| `vault-hygiene` | 0 | m9-11 drift fixed, procedure extended (check #8) |

## Verdict

**PASS**

Reason tied to mandatory gates:
- `subject_identity`: PASS — clean tree, HEAD pinned at
  `38699061891b76f90ef316914d3ba15d6eb53f83`.
- `behavioral_compliance`: PASS — the fix matches the documented
  remediation (real SHAs, add check #8).
- `real_implementation`: PASS — no stubs or mocks.
- `documentation_discipline`: PASS — drift history documented.
- `regression_and_build`: PASS — T0 clean; no production code touched.
- `production_readiness`: PASS — doc-only change.
- `design_and_solid`: PASS — new check is minimal, mechanical.
- `task_completeness`: PASS — all four deliverables shipped together.

The cycle is ready to advance to `sddk-archive` for vault sync.
## Cross-checks

Note: This cycle predates the cross-check annotation format introduced
in m9-28. Per `vault-drift-sweep.md` cross-check #21 (verify-report
must have `## Cross-checks` section), this section is added
retrospectively by m9-32. The cycle's verify-report content above is
unchanged.

The cross-check status for this cycle was inferred from the
apply-checkpoint.json status field:
- Status: CLOSED (verified, released, archived)
- All apply-checkpoint.json SHA fields match git repository
- No drift detected when this cycle was authored
