# Release Report — m9-80-property-policy-ownership (pending release)

**Path**: A-min
**Cycle**: m9-80-property-policy-ownership
**Note**: Release-receipt synthesised 2026-09-14T08:43Z at the verify phase.
Merge to `main` and tag publication pending release phase.

## Subject

| Base | Head (verified) | Tag (predicted) | CWD | Verified at |
|---|---|---|---|---|
| `82e219f` | `ccf8811` | `v0.7.82` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-14T08:40Z |

## Summary

| Verdict | Mode | Path | Required scenarios | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|---|
| **PASS** | normal | A-min | 6 (REQ-M9-80-01 × 3 + REQ-M9-80-02 × 3 + REQ-M9-80-03 × 3) | 9/9 | 0 | 0 |

## Cross-checks

- CC#4: `python3 scripts/regen_manifest_index_shas.py` reports "nothing to do
  (77 manifests already correct)"; 10 archive-manifest.md files in the cycle
  diff were regenerated to a fixpoint at the m9-80 row addition.
- CC#28: Base SHA = `82e219f` will be in release-receipt (created at release time).
- CC#42: peel-match will be populated when the tag is published. Expected:
  `v0.7.82 → ccf8811` (clean peel match since this cycle has no merge drift
  like the m9-78 backfill). release-receipt.md is intentionally NOT created
  at verify phase (CC#42 would trip on a missing git tag); it is created
  in the release phase once the tag is published.
- CC#48: `bash scripts/check_vault_drift.sh` PASS (48 python CCs + 7 bash CCs).
- CC#51: cycle-artifacts folder exists at
  `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos/cycle-artifacts/p-3416cfb8288f8964/m9-80-property-policy-ownership/`
  with this report, the implementation-receipt, the merge-receipt, the
  verify-findings, the verify-report, and the apply-checkpoint. The
  release-receipt is created at release time.
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
- All 6 mechanical spec scenarios verified (see verify-report.md for the
  table).
- Cycle branch `feat/m9-80-property-policy-ownership` will be deleted after
  release, per local branch hygiene convention.
