# Archive Manifest — m9-34

| Field | Value |
|---|---|
| Cycle | m9-34-verify-findings-and-archive-manifest-schema |
| Head SHA | `5cfa91be296065ab4ca19c38bb0b06a859260627` |
| Base SHA | `7ffa57ab6bed79c9300b35d81a323e0f874746b2` |
| Tag | `v0.7.32` |
| Path | B-direct |
| Date | 2026-09-12 |

## Summary

Two structural drift classes closed:

1. **verify-findings.json subject schema** (23 files: m9-05..m9-27).
   Each cycle's `subject` dict had `head_sha` but no `base_sha`.
   Added `subject.base_sha` from apply-checkpoint.json.

2. **archive-manifest.md header** (10 files: m9-01..m9-10).
   The m9-01..m9-10 archive-manifests used `| Published SHA |` instead
   of `| Head SHA |`. Added `| Head SHA |` as an alias with the same
   value so cross-references match m9-11+.

Both are zero-content drift closes; the SHA values were already known
from apply-checkpoint.json.

Added cross-check #26 to vault-drift-sweep.md to enforce both schemas.

## Cross-checks

- C26: pass (after fixes applied)
- C1-C25: pass

## Evidence bindings

- `cycle-artifacts/p-3416cfb8288f8964/m9-34-verify-findings-and-archive-manifest-schema/apply-checkpoint.json` → sha256: `7a56c4c406e917c3f9dd599e963af29415ae9cd1a5cafc769d969a593a0464cb`
- `cycle-artifacts/p-3416cfb8288f8964/m9-34-verify-findings-and-archive-manifest-schema/merge-receipt.md` → sha256: `f08b49f8ac38ed23ccf9476e63fa5120ccdf2a8c605bb41d490f2ef44e411502`
- `cycle-artifacts/p-3416cfb8288f8964/m9-34-verify-findings-and-archive-manifest-schema/release-receipt.md` → sha256: `2490efd59e4ca2a1146f1ad7a6eb76fde41dd603120395a4b146e5244b0b6578`
- `cycle-artifacts/p-3416cfb8288f8964/m9-34-verify-findings-and-archive-manifest-schema/release-report.md` → sha256: `c2acdd2548af8dcb8439e57a601e82b1cb8250d39a3810cc6c9bb6a597cb89f3`
- `cycle-artifacts/p-3416cfb8288f8964/m9-34-verify-findings-and-archive-manifest-schema/verify-findings.json` → sha256: `6d34aac4034598af790e50b45d4bf93ec63916701242cfad2e41491663f69b2b`
- `cycle-artifacts/p-3416cfb8288f8964/m9-34-verify-findings-and-archive-manifest-schema/verify-report.md` → sha256: `1a2d8a92db3b760be8462f654b5e8b35b288a5faa8a1c7887c8fea614d47d0c8`
