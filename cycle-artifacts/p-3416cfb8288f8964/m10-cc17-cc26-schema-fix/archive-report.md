# Archive Report — m10-cc17-cc26-schema-fix

**Cycle**: `p-3416cfb8288f8964/m10-cc17-cc26-schema-fix`
**Path**: B-direct
**Status**: CLOSED
**Generated**: 2026-09-15T10:14Z

## Executive Summary

B-direct cycle to close CC#17 + CC#26 drift (3 + 6 = 9 drift lines). Normalized `verify-findings.json` schema across 8 cycle artifacts by adding a `subject` dict with `head_sha`, `base_sha`, `cycle_id`, `branch`, `route`. Also fixed an invalid JSON escape in `m9-66-bash-cc-meta-check/verify-findings.json` that was crashing the CC sweep script. Closed in single commit `63a7061c` (10 files, +212/-149). Tag `v0.7.107` peels to merge commit `ab0b8731`. No Rust code touched.

## Cycle acceptance (against the B-direct intent)

| Requirement | Status |
|---|---|
| CC#17 reports 0 drift lines after cycle | ✓ (was 3; now 0) |
| CC#26 Part A reports 0 drift lines after cycle | ✓ (was 6; now 0) |
| All 8 verify-findings.json files have `subject` dict | ✓ |
| All 8 verify-findings.json have `subject.head_sha` AND `subject.base_sha` | ✓ |
| m9-66 invalid JSON escape fixed | ✓ |
| CC#4 fixpoint cascade clean | ✓ (2 archive-manifest SHA rows rewritten) |
| cargo fmt/clippy clean | ✓ |
| cargo tests pass | ✓ (no source touched; 422 tests pass) |
| Tag v0.7.107 on merge commit | ✓ |
| HEAD = origin/main | ✓ |
| 7 SDDK gate receipts emitted | ✓ |

## Files Inventory

| Bucket | Count | Notes |
|---|---|---|
| Cycle artifacts (this directory) | 7 | merge-receipt, release-receipt, implementation-receipt, releases-report (omitted), archive-manifest, archive-report, reports/cierre.html |
| Source files touched (in cycle commit) | 8 | verify-findings.json × 8 in cycle-artifacts/ |
| archive-manifest.md files rewritten (CC#4 cascade) | 2 | m9-81, m9-82 |

## Out of scope (named as next carry-forward)

Discovered during drift sweep re-evaluation after fixing m9-66's bad JSON escape:

- **CC#30**: 10 drift lines
- **CC#34**: 10 drift lines
- **CC#35**: 6 drift lines
- **CC#36**: 2 drift lines
- **CC#41**: 6 drift lines
- **CC#43**: 7 drift lines

Total residual: **41 drift lines across 6 CCs**. All pre-existing schema/section issues on legacy m9 cycle artifacts (was hidden by the early crash). Recommended next cycle: `m10-cc30-cc34-cc35-cc36-cc41-cc43-schema-fix` (A-min).

## Drift report final line

```
DRIFT detected (CC#48):
DRIFT: CC#30 reported 10 drift lines
DRIFT: CC#34 reported 10 drift lines
DRIFT: CC#35 reported 6 drift lines
DRIFT: CC#36 reported 2 drift lines
DRIFT: CC#41 reported 6 drift lines
DRIFT: CC#43 reported 7 drift lines
```

(CC#17, CC#26, CC#18, CC#4, CC#39 all clean.)

## Notes

- The cycle's spawn delegate (MiniMax-M2.7-highspeed) failed on the upstream API endpoint, so the orchestrator (mouse) executed the apply phase directly. SDDK lifecycle was registered post-merge as in m10-vault-handoff-relocate.
- Subject SHAs for m9 cycles extracted via `git log --all --merges --pretty='%H %P' --grep <cycle_id>`. First parent of merge commit = base_sha.
- `cycle-artifacts/p-3416cfb8288f8964/handoffs/verify-findings.json` is a documentation marker for the `handoffs/` dir (not a real cycle); synthetic SHA = current main HEAD `09eae57e`.

## Tag line in trunk

```
v0.7.103 → 3f7abc351974835a215767b9f491dad3aef3474e  (m10-ms-cap-discovery)
v0.7.104 → 11efb266bc10dd21f48e606b403740cefaeb6da2  (m10-ms-cap-discovery-followup)
v0.7.105 → fa92a6ee40aea67a9dbac9301a14fafd3c15b7af  (m10-vault-last-updated-backfill)
v0.7.106 → 340d64d969ba3df936f4c3e73720e8192d980d73  (m10-vault-handoff-relocate)
v0.7.107 → ab0b873179e18ca3c9ccc04d47abe1d78bcb5e17  (m10-cc17-cc26-schema-fix, this cycle)
```
