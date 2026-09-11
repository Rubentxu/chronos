# Merge Receipt: m9-02-events-side-table

## Cycle

| Field | Value |
|---|---|
| cycle_id | m9-02-events-side-table |
| path | A-lite |
| base | 48a9cff54ec165987063705d5d4d3af453fcb6b3 |
| head | 1a8d104ba2da883b40bd424cdc079b344f7ed63c |
| branch | main |

## Git Evidence

```bash
# Local HEAD after merge
$ git rev-parse HEAD
1a8d104ba2da883b40bd424cdc079b344f7ed63c

# Remote origin/main after push
$ git rev-parse origin/main
1a8d104ba2da883b40bd424cdc079b344f7ed63c

# SHA equality
$ test "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)"
# exit 0 — PASS
```

## Push Receipt

```
To github.com:Rubentxu/chronos.git
   48a9cff..1a8d104  main -> main
```

## Commits Merged

| SHA | Subject |
|---|---|
| a765d56 | docs(m9-02): scoping — bundle events side table (lazy-loading + legacy-compat) |
| b2a2455 | docs(m9-02): spec — bundle events side table (lazy-loading + legacy-compat) |
| 3d758f3 | docs(m9-02): tasks — bundle events side table (4 phases, 16 tasks) |
| 126d4e5 | docs(m9-02): design — bundle events side table (D1–D8, atomic save, chokepoint loader) |
| 9fc2211 | feat(chronos-store): schema_version=2 + BUNDLE_EVENTS_CHUNK_SIZE + events_count (m9-02) |
| 624c8f2 | feat(chronos-store): counterexample_bundle_events side table + save/load/count (m9-02) |
| 19bbf73 | feat(chronos-services,chronos-cli): events_count + bundle_events_or_legacy (m9-02) |
| e410f44 | test(chronos-store,chronos-services,chronos-cli): 14 m9-02 tests (m9-02) |
| a62c3b1 | fix(m9-02): correct events_count wire after side-table migration and silence clippy |
| 1a8d104 | chore(m9-02): rustfmt collapses use block after unused-import removal |

## Contract Compliance

- Rule 0: `HEAD == origin/main` ✓
- Rule 1: Local verification preceded publication ✓
- Rule 2: Direct main push is the publication effect ✓
- Rule 4: merge-receipt recorded at 2026-09-11T20:34:28Z ✓
