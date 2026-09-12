# Archive Manifest — m9-59-apply-checkpoint-remote-tag-canonical

## Summary

m9-59 closes the "missing `remote_tag` field" drift class. 31 apply-checkpoint.json files were normalized: 16 had legacy `tag` field (m9-03..m9-18, pre-m9-19 era), renamed to canonical `remote_tag`. 15 had neither (m9-19..m9-33, mid-era), backfilled from cycles/index.md. Cross-check #50 added.

## Cycle

| Campo | Valor |
|---|---|
| Cycle | m9-59-apply-checkpoint-remote-tag-canonical |
| Base SHA | `34341fa72f8ad003ce687493588e8b58b34d3dfb` |
| Head SHA | `d54747a6eeaeb828fbba9c7f09a219656e96561d` |
| Path | B-direct |
| Date | 2026-09-12T18:46Z |
| Branch | `fix/m9-59-apply-checkpoint-remote-tag-canonical` |
| Tag | `v0.7.61` |
| Tag peel SHA | `d54747a6eeaeb828fbba9c7f09a219656e96561d` |
| Peel match | `d54747a6eeaeb828fbba9c7f09a219656e96561d` |
| Status | CLOSED |

## Evidence bindings

- **`apply-checkpoint.json`**: `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-M9-59-APPLY-CHECKPOINT-MISSING-REMOTE-TAG]`
- **`verify-findings.json`**: 1 finding (FIND-M9-59-APPLY-CHECKPOINT-MISSING-REMOTE-TAG), verdict `pass_with_findings`
- **`verify-report.md`**: Subject, Findings, Files Inventory (40 rows), Cross-checks (#50), Verification
- **`merge-receipt.md`**: `Base SHA | 34341fa…`, `Head SHA | d54747a6eeaeb828fbba9c7f09a219656e96561d`
- **`release-receipt.md`**: `Remote tag | v0.7.61`, `Peel match | d54747a6eeaeb828fbba9c7f09a219656e96561d`

## Tangential modifications

31 apply-checkpoint.json files modified:

| Group | Count | Change |
|---|---|---|
| m9-03..m9-18 | 16 | `tag` → `remote_tag` (rename) |
| m9-19..m9-33 | 15 | `remote_tag` backfilled from cycles/index.md |

1 maintenance doc modified:

| File | Change |
|---|---|
| `vault-drift-sweep.md` | Added CC#50 |

## Cross-checks

- C1-C49: pass
- C50: pass (after fix)
- C48 meta-check: pass (0 DRIFT lines across all 50 CCs)
