# Release Report — m10-cc17-cc26-schema-fix

**Status**: success
**Cycle**: `p-3416cfb8288f8964/m10-cc17-cc26-schema-fix`
**Path**: B-direct
**Base SHA**: `09eae57e93cb0ce8f705da0c7734a62de80ca408`
**Main SHA (post-merge)**: `ab0b873179e18ca3c9ccc04d47abe1d78bcb5e17`
**Tag**: `v0.7.107`
**Date**: 2026-09-15T10:09Z

## Summary

B-direct cycle to close CC#17 + CC#26 drift. Eight `verify-findings.json` files normalized to add a `subject` dict with `head_sha`, `base_sha`, `cycle_id`, `branch`, `route`. m9-66's bad JSON escape also fixed (was crashing the CC sweep script). Single reviewable commit `63a7061c` (10 files, +212/-149). Tag `v0.7.107` peels to merge commit `ab0b8731` per AGENTS.md §5 fixpoint-cascade workaround.

## Diff scope

| Category | Count | Notes |
|---|---|---|
| verify-findings.json schema normalizations | 8 | handoffs + 3 m10 + 4 m9 |
| JSON escape fixes | 1 | m9-66 |
| archive-manifest.md SHA row rewrites (CC#4 cascade) | 2 | m9-81, m9-82 |
| Rust source changes | 0 | n/a |

## Acceptance gates (inline)

- **T0 fmt**: exit 0
- **T0 clippy** (mcp + services): exit 0
- **T2 cargo test** (mcp + services): 422 pass / 0 fail (no source touched)
- **T2 regen-manifest-index-shas.py --check**: clean (98 manifests) after one cascade run
- **CC#17**: 0 drift lines (was 3) ✓
- **CC#26 Part A**: 0 drift lines (was 6) ✓
- **CC#4 fixpoint**: clean ✓

## Notes

- The cycle's spawn delegate (MiniMax-M2.7-highspeed) failed on the upstream API endpoint, so the orchestrator executed the apply phase directly.
- Subject SHAs for m9 cycles extracted via `git log --all --merges --pretty='%H %P' --grep <cycle_id>`. First parent of merge commit = base_sha.
- `cycle-artifacts/p-3416cfb8288f8964/handoffs/verify-findings.json` is a doc dir marker; synthetic SHA = current main HEAD.

## Discovered residual (next carry-forward)

41 drift lines across CC#30/34/35/36/41/43, all pre-existing schema issues on legacy m9 cycle artifacts. Hidden until m9-66's bad JSON escape was fixed in this cycle, which let the CC sweep script run past CC#17/CC#26 to evaluate later checks.
