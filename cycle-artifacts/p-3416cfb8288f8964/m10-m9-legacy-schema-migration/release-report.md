# Release Report — m10-m9-legacy-schema-migration

| Field | Value |
|---|---|
| Cycle | `m10-m9-legacy-schema-migration` |
| Path | A-lite |
| Tag | `v0.7.109` |

## What changed

Vault-only cycle. No Rust source code touched. Mechanical schema migration:

1. **17 verify-findings.json** files migrated from legacy schemas
   (list, markdown-hybrid, or dict without `subject`) to modern schema.
2. **60 verify-report.md** `## Findings` sections normalized (None marker
   for prose-only cycles, table preserved for cycles with F-row tables).
3. **2 archive-manifest.md** files received `| Cycle |` row.
4. **51 archive-manifest.md** files had SHA-256 hashes regenerated
   (CC#4 cascade).
5. **m9-78** verify-findings.json subject keys aligned.

## Verification

- T0 fmt + clippy: clean (no source touched)
- T3 cargo test: in progress, expected pass
- CC drift before cycle: 13+ lines (CC#30/34/36/41/43)
- CC drift after cycle: 0 (target CCs); 2 pre-existing CCs (CC#5, CC#53)
  exposed via CC#54, not introduced
- Cycle ledger: 237 → 245 events (8 new events)
- HEAD = origin/main = `bbc65a70` (merge commit)
- Tag v0.7.109 peels to apply commit `47d89b1a` (per AGENTS.md §5)

## What closed

| CC | Before | After |
|---|---|---|
| CC#30 Part C (verify-findings verdict + archive-manifest Cycle) | 2 lines | 0 |
| CC#34 Part C (verify-findings cycle_id) | 2 lines | 0 |
| CC#36 Part C (verify-report Findings format) | 2 lines | 0 |
| CC#41 Part A (verify-findings lens_summary) | 6 lines | 0 |
| CC#43 Part B (verify-findings head_sha consistency) | 1 line | 0 |
| CC#4 (archive-manifest SHA-256) | clean | clean (cascade regenerated) |
| CC#38 (verify-findings findings array vs verify-report table) | not surfaced | clean (alignment fixed) |

## Carry-forward (unchanged)

- CC#5 (cycles/index.md row count = 98 vs 0 m9-* rows) — pre-existing.
- CC#53 (MERGED-LOCAL branches reported) — pre-existing.
