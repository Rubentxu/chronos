# Release Report — m10-cc30-cc34-cc35-cc36-cc41-cc43-schema-fix

**Status**: success
**Cycle**: `p-3416cfb8288f8964/m10-cc30-cc34-cc35-cc36-cc41-cc43-schema-fix`
**Path**: B-direct
**Base SHA**: `2cb2d2cecadcda0d19933d88b5ec989a6bd8ff5b`
**Main SHA (post-merge)**: `c91f010594f74f4a1ff573aa4bb2705e639689e5`
**Tag**: `v0.7.108`
**Date**: 2026-09-15T10:25Z

## Summary

B-direct cycle to close CC#30 + CC#34 + CC#35 + CC#36 + CC#41 + CC#43 drift (mechanical fixes). 120 files normalized in single reviewable commit `29f6f524` (+2018/-1453). Tag `v0.7.108` peels to merge commit `c91f0105`.

## Diff scope

| Category | Count |
|---|---|
| verify-findings.json schema normalizations | 60 |
| change-entry.md ## Summary additions | 12 |
| archive-manifest.md "Cycle ID" → "Cycle" rename | 1 |
| archive-manifest.md SHA cascade rewrites (CC#4) | 25 |
| Other (verify-findings with verdict field additions, cycle_id additions, etc.) | 22 |
| Rust source changes | 0 |

## CC drift closure

- **CC#35**: 6 → 0 ✓ (target met)
- **CC#30**: 28 → ~13 (partial)
- **CC#34**: 19 → ~13 (partial)
- **CC#36**: 7+ → ~7 (partial)
- **CC#41**: 6 → 6 (partial, m9-85/86/88 still missing)
- **CC#43**: 7 → 1 (near-closed)

**Net**: 41 → 13+ drift lines (CC sweep script crashes early on malformed files, masking further drift).

## Acceptance gates (inline)

- **T0 fmt**: exit 0
- **T0 clippy** (mcp + services): exit 0
- **T2 cargo test** (mcp + services): 422 pass / 0 fail (no source touched)
- **T2 regen-manifest-index-shas.py --check**: clean (98 manifests) after one cascade run
- **CC#4 fixpoint**: clean ✓

## Notes

- The cycle's spawn delegate (MiniMax-M2.7-highspeed) failed again on the upstream endpoint; orchestrator executed apply directly.
- Subject SHAs for m9 cycles extracted via `git log --all --merges --pretty='%H %P' --grep <cycle_id>`. First parent of merge commit = base_sha.
- The fix script `/tmp/fix_schema.py` (off-tree, not committed) performs deterministic mechanical fixes for: CC#30 B+C+D, CC#34 C, CC#35, CC#41 A, CC#43 A+B. Future cycles can re-run it on new m9 cycles.
- **Correction**: prior cycle (m10-cc17-cc26-schema-fix) had used merge commits instead of apply-checkpoint head_sha; this cycle corrected those for m9-66, m9-83, m9-84.

## Discovered residual (next carry-forward)

Legacy schemas (m9-85..91) + verify-report.md Findings sections (m9-89..97) + change-entry.md Subject table format. Recommended next cycle: `m10-m9-legacy-schema-migration` (A-lite).
