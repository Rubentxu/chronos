# Release Report — m9-80-property-policy-ownership

**Path**: A-min
**Cycle**: m9-80-property-policy-ownership
**Tag**: v0.7.82
**Merge commit**: 7874e5c8e972172c024b07f60746f0e06df92f9d

## Subject

| Base | Head (verified) | Tag | Tag SHA | Tag peel | CWD | Verified at |
|---|---|---|---|---|---|---|
| `82e219f` | `8012342` | `v0.7.82` | `c77333500da4ebb8f46e3b85cb0f9bfbf7bd4bd5` | `7874e5c8e972172c024b07f60746f0e06df92f9d` (merge commit) | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-14T08:40Z |

## Summary

| Verdict | Mode | Path | Required scenarios | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|---|
| **PASS** | normal | A-min | 6 (REQ-M9-80-01 × 3 + REQ-M9-80-02 × 3 + REQ-M9-80-03 × 3) | 9/9 | 0 | 0 |

## Cross-checks

- CC#4: `python3 scripts/regen_manifest_index_shas.py` reports "nothing to do
  (77 manifests already correct)"; 10 archive-manifest.md files in the cycle
  diff were regenerated to a fixpoint at the m9-80 row addition.
- CC#28: Base SHA = `82e219f` in release-receipt.md.
- CC#42: `v0.7.82 → 7874e5c` (clean peel match: tag_peel == merge_commit_sha).
- CC#48: `bash scripts/check_vault_drift.sh` PASS (48 python CCs + 7 bash CCs).
- CC#51: cycle-artifacts folder exists at
  `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos/cycle-artifacts/p-3416cfb8288f8964/m9-80-property-policy-ownership/`
  with this report, the implementation-receipt, the merge-receipt, the
  release-receipt, the verify-findings, the verify-report, and the
  apply-checkpoint.
- CC#55: Files Inventory present in verify-report.md (18 files listed).

## Notes

- The cycle is a cross-crate ownership refactor (A-min, 1 apply phase each on
  domain + services). The spec was revised at base (`82e219f`) from rev 1
  ("byte-for-byte move") to rev 2 (layered split) after recon showed the four
  functions reference services-layer wire types. The implementation followed
  rev 2: domain returns `PropertyHypothesisOutcome`-family types; services
  wraps via `From` impls.
- No wire / protocol change. Patch bump `v0.7.81` → `v0.7.82` per tag
  convention.
- 552 tests pass across all tiers (264 services + 149 domain + 103 native
  serial + 13 hypothesis_test + 23 sandbox smoke = 552).
- All 6 mechanical spec scenarios verified PASS (see verify-report.md).
- Merged with `--no-ff` to preserve cycle branch topology; branch deletion
  happens in the archive phase.
