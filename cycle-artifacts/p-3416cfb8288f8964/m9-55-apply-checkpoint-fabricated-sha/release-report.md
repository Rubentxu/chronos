# m9-55: Apply-checkpoint base_sha fabrication fix

## Path

B-direct

## Subject

Fix fabricated `base_sha` in m9-38 cycle artifacts (7 files) and add cross-check #47 to catch future SHA-fabrication drift class.

## Files changed

| File | Change |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-38-verify-report-title-format-normalize/apply-checkpoint.json` | `base_sha`: `6bc6781465f9dff5a59bd4d8e8a99930dba3e7e5` → `6bc67812d66548a3e0ee48f5323d45c4a1ed13d8` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-38-verify-report-title-format-normalize/merge-receipt.md` | Base SHA updated |
| `cycle-artifacts/p-3416cfb8288f8964/m9-38-verify-report-title-format-normalize/release-receipt.md` | Base SHA updated |
| `cycle-artifacts/p-3416cfb8288f8964/m9-38-verify-report-title-format-normalize/verify-findings.json` | `subject.base` updated |
| `cycle-artifacts/p-3416cfb8288f8964/m9-38-verify-report-title-format-normalize/verify-report.md` | `## Subject` table `Base` cell updated |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-38-verify-report-title-format-normalize/change-entry.md` | `## Subject` bullet `base_sha` updated |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-38-verify-report-title-format-normalize/archive-manifest.md` | `Base SHA` row updated |

| New files | Purpose |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-55-*/` | m9-55 cycle artifacts (apply-checkpoint, verify-findings, verify-report, merge-receipt, release-receipt, release-report) |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-55-*/change-entry.md` | m9-55 knowledge artifact |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-55-*/archive-manifest.md` | m9-55 archive manifest |
| `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` | cross-check #47 added |
| `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | m9-55 row added |
| `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | timestamp updated |

## Cross-checks

| ID | Description | Status |
|---|---|---|
| #47 | apply-checkpoint.json `base_sha` must exist in git (`git cat-file -t <sha>` returns `commit`) | closed by m9-55 |

## History

m9-55 closes the SHA-fabrication drift class. Pre-existing prior sweeps validated SHA *format* (length 40, hex chars) but did not validate *reachability*. Detection during m9-54 cleanup.
