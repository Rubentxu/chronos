# Archive Manifest: m9-19

| Field | Value |
|---|---|
| Cycle | m9-19-route-main-sha-and-empty-folder-drift |
| Head SHA | `ec58934689b73a6a78cb65b8ece96c6cdda13b35` |
| Tag | `v0.7.17` |
| Tag peel | `ec58934689b73a6a78cb65b8ece96c6cdda13b35` |
| Peel match | true |
| Archived at | 2026-09-12T11:11:00Z |
| Path | B-direct |
| Cross-check added | #12 |

## Archived artifacts

### Cycle artifacts (`cycle-artifacts/p-3416cfb8288f8964/m9-19-route-main-sha-and-empty-folder-drift/`)

| File | SHA-256 |
|---|---|
| `apply-checkpoint.json` | `bca1f6720c7b40139e5b7d3fb225e9e194bfa7fa28215e2ed53171d53726a02b` |
| `merge-receipt.md` | `e5002bdff47c2e5686ea1ebade64fe3f3063658bb6d03461a915c564b3143cb7` |
| `release-receipt.md` | `c3ec181cf4466e32568dc8b9453c5b015519be8bfd6e25d183f975105e547066` |
| `release-report.md` | `5f20f8e2c7fa77dd7158a90baa4c62a69d8f59cbd47f3340ef5dae927bee6c51` |
| `verify-findings.json` | `0dfbe52028f5740df5a57677f42e16827267587439faa2c5d6c510a68c2925ac` |
| `verify-report.md` | `2509c6b864aa85e0f06c4fd96e933153b57f68ac49c42e67103c13659f9a3a66` |

### Knowledge artifacts (`.sddk-knowledge/p-3416cfb8288f8964/changes/m9-19-route-main-sha-and-empty-folder-drift/`)

| File | SHA-256 |
|---|---|
| `change-entry.md` | `798e75e3040a7f0d3c212056814924ac1b875a2ab3f98a59a542dce8932ad286` |

### Modified cycle artifacts

The following prior-cycle artifacts were modified (route field +
main_sha convention):

- `cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/apply-checkpoint.json` (route: local → B-direct)
- `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/apply-checkpoint.json` (route: local → B-direct)
- `cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/apply-checkpoint.json` (route: local → B-direct)
- `cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/apply-checkpoint.json` (route: local → B-direct)
- `cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/apply-checkpoint.json` (route: local → B-direct)
- `cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/apply-checkpoint.json` (route: local → B-direct)
- `cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/apply-checkpoint.json` (route: local → B-direct)
- `cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/apply-checkpoint.json` (route: local → B-direct)
- `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/apply-checkpoint.json` (main_sha: base_sha → head_sha)
- `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/apply-checkpoint.json` (main_sha: base_sha → head_sha)
- `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/apply-checkpoint.json` (main_sha: base_sha → head_sha)

### Removed artifacts

- `cycle-artifacts/p-3416cfb8288f8964/m9-11-m9-04-findings-closed-drift-fix/` (empty folder, removed via rmdir)

## SHA verification

```bash
git rev-parse v0.7.17^{}  # → ec58934689b73a6a78cb65b8ece96c6cdda13b35
git cat-file -p v0.7.17   # → object ec58934689b73a6a78cb65b8ece96c6cdda13b35 type commit
```
