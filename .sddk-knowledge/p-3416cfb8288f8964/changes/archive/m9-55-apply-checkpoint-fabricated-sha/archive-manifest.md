# Archive Manifest — m9-55-apply-checkpoint-fabricated-sha

## Cycle

| Campo | Valor |
|---|---|
| Cycle | m9-55-apply-checkpoint-fabricated-sha |
| Base SHA | `cbb93847228a9062de8e093dfe452c257233228c` |
| Head SHA | `6ca0afddccb018c0d66bfbb35fa44dbc58ec2db4` |
| Path | B-direct |
| Date | 2026-09-12T17:27Z |
| Branch | `fix/m9-55-apply-checkpoint-fabricated-sha` |
| Tag | `v0.7.53` |
| Tag peel SHA | `6ca0afddccb018c0d66bfbb35fa44dbc58ec2db4` |
| Peel match | `6ca0afddccb018c0d66bfbb35fa44dbc58ec2db4` |
| Status | CLOSED |

## Evidence bindings

- **`apply-checkpoint.json`**: `cycle-artifacts/p-3416cfb8288f8964/m9-55-*/apply-checkpoint.json` — `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-M9-55-FABRICATED-BASE-SHA]`
- **`verify-findings.json`**: 1 finding (FIND-M9-55-FABRICATED-BASE-SHA), verdict `passed`
- **`verify-report.md`**: Subject table, Findings section, Files Inventory (15 lines), Cross-checks (#47), Verification (4 rows), History
- **`merge-receipt.md`**: `Base SHA | cbb9384…`, `Head SHA | 6ca0afddccb018c0d66bfbb35fa44dbc58ec2db4`
- **`release-receipt.md`**: `Remote tag | v0.7.53`, `Peel match | 6ca0afddccb018c0d66bfbb35fa44dbc58ec2db4`
- **`release-report.md`**: `# m9-55: Apply-checkpoint base_sha fabrication fix`, Path B-direct, Subject, Files (13 changed), Cross-checks (#47)

## Tangential modifications

7 m9-38 files modified:

| File | Fix |
|---|---|
| `m9-38/apply-checkpoint.json` | base_sha corrected |
| `m9-38/merge-receipt.md` | Base SHA corrected |
| `m9-38/release-receipt.md` | Base SHA corrected |
| `m9-38/verify-findings.json` | subject.base corrected |
| `m9-38/verify-report.md` | `## Subject` Base cell corrected |
| `.sddk-knowledge/.../m9-38/change-entry.md` | base_sha bullet corrected |
| `.sddk-knowledge/.../m9-38-archive/archive-manifest.md` | Base SHA row corrected |

## Cross-checks

| ID | Status |
|---|---|
| #47 | closed by m9-55 |
| #5 | holds |
| #45 | holds for m9-38 (e8f805f vs v0.7.36) |

## Date

2026-09-12T17:27:00Z

## Path

`.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-55-apply-checkpoint-fabricated-sha/archive-manifest.md`
