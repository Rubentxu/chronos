# Archive Manifest — m10-cc17-cc26-schema-fix (initial)

**Cycle**: `p-3416cfb8288f8964/m10-cc17-cc26-schema-fix`
**Path**: B-direct
**Status**: RELEASED → CLOSED (post `archive.complete`)
**Base SHA**: `09eae57e93cb0ce8f705da0c7734a62de80ca408`
**Merged SHA**: `ab0b873179e18ca3c9ccc04d47abe1d78bcb5e17`
**Tag**: `v0.7.107`
**Subject binding**: tag peel `v0.7.107^{commit}` = `ab0b873179e18ca3c9ccc04d47abe1d78bcb5e17` ✓ matches main HEAD

## Artifacts

| Path | SHA-256 |
|---|---|
| `merge-receipt.md` | `df9f160a5c9b96bac24741a2bebe3f7fb08a71274e9661c0f1cc60a2d1bb43d7` |
| `implementation-receipt.md` | `81c769c2ac2399dae91696641ec0faea65c3368b51784dcfd243827a90e8365d` |
| `release-receipt.md` | `f2b828d0324ba1a9255acbb5c6724d423d58e373212645031e0f17b501087885` |
| `archive-report.md` | `4aaaac25b85b3bae1045b17d9262a1cacbb24c4f6aa49222a39079ae5d03c7c0` |
| `reports/cierre.html` | `9227bb4c19bffaa003cb9b2bfe0744f729196989757d942efab7c1df2064b7c9` |
| `verify-findings.json` | (this file's SHA is recorded in cycles/index.md via the cascade; not enumerated here to avoid circular reference) |

## Restore

```bash
git checkout ab0b873179e18ca3c9ccc04d47abe1d78bcb5e17 -- cycle-artifacts/p-3416cfb8288f8964/m10-cc17-cc26-schema-fix
```

## Provenance

| Source | Value |
|---|---|
| Branch | `feat/m10-cc17-cc26-schema-fix` |
| Feature HEAD | `63a7061c8e22c6fa9618b70ff821ddda89e29876` |
| Merge strategy | `--no-ff` (per AGENTS.md §5 CC#42 fixpoint-cascade workaround) |
| Subject binding | main HEAD == tag peel ✓ |
| Diff size | 10 files, +212/-149 |
| Trunk | `09eae57e..ab0b8731` |
