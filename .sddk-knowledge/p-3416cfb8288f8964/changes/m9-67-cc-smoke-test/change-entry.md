# Change: m9-67 CC smoke test for drift detection

## Summary

Added `scripts/smoke_test_ccs.sh`, a synthetic-drift injection test that exercises the four most failure-prone vault drift CCs (CC#4, CC#39/CC#5, CC#46, and the CC#48+CC#54 meta-checks) and verifies the meta-check chain still catches each. Closes the "fix a broken CC, then fix what it would have caught" pattern observed twice this session: m9-65 (49 stale branches because CC#46 was missing milestone prefixes) and m9-66 (54 stale SHAs because CC#4 had a broken awk regex).

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-67-cc-smoke-test` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `67b3d76bb6e90accefb15cb1f3f55f1b4a52b4dc` |
| Head SHA | `ef21e1fef358f63cc73e320688ec566c0806c378` |
| Tag | `v0.7.69` |

## Subject

- base_sha: `67b3d76bb6e90accefb15cb1f3f55f1b4a52b4dc`
- head_sha: `ef21e1fef358f63cc73e320688ec566c0806c378`
- cycle: m9-67
- branch: `feat/m9-67-cc-smoke-test`
- date: 2026-09-13
- tag: `v0.7.69`
- findings_closed: 1 (FIND-M9-67-NO-CC-SMOKE-TEST)
- findings_introduced.no_action: 0

## Files changed

- (new) `scripts/smoke_test_ccs.sh` — synthetic-drift injection test (250 lines)
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-67-cc-smoke-test/apply-checkpoint.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-67-cc-smoke-test/verify-findings.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-67-cc-smoke-test/verify-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-67-cc-smoke-test/merge-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-67-cc-smoke-test/release-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-67-cc-smoke-test/release-report.md`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-67-cc-smoke-test/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-67-cc-smoke-test/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-67 row added, Total cycles 66→67)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last archive bumped)

## Cross-checks

CC#1..CC#54: pass (no drift). Self-test: 4 smoke tests pass (~35s wall time).

## Follow-ups (deferred)

- **Bounded join unit test** (deferred from m9-62): 10s runtime blocker; may need different test strategy.
- **CC for `## Files Inventory` in verify-report**: cosmetic; 22 cycles m9-32..m9-53 still missing.
- **5 local + 19 remote not-merged branches triage** (preserved from m9-65): needs human review.
- **Sandbox test warm-up ordering**: `test_session_start_via_v2_then_session_stop_via_v2` in probe_lifecycle fails alone, passes with full file. Likely binary warm-up.
