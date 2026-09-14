# Release Report — m9-81-counterexample-table-classifier

**Path**: B-direct
**Cycle**: m9-81-counterexample-table-classifier
**Tag**: v0.7.83
**Merge commit**: fdc5accf64be1fcf780913243aec0496ad48e7fe

## Subject

| Base | Head (verified) | Tag | Tag peel | CWD | Verified at |
|---|---|---|---|---|---|
| `45b53df` | `a3f59ea` | `v0.7.83` | `fdc5accf64be1fcf780913243aec0496ad48e7fe` (merge commit) | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-14T09:30Z |

## Summary

| Verdict | Mode | Path | Required scenarios | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|---|
| **passed** | normal | B-direct | 10 (REQ-M9-81-01 × 7 + REQ-M9-81-02 × 3) | 4/4 | 0 | 0 |

m9-81 closes FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION by
routing 6 production read-path sites in
`crates/chronos-store/src/counterexample_storage.rs` through the canonical
`chronos_store::table_error::classify_read_table_error()` helper that
m9-72 introduced for `cas.rs` and `storage.rs`. Behaviour-preserving
refactor: 74 / 0 lib unit test counts match before and after.

## Source delta

- `crates/chronos-store/src/counterexample_storage.rs`: 11 insertions /
  13 deletions (net -2 lines). 6 sites refactored. Imports:
  `TableError` removed (no longer named); `classify_read_table_error`
  added.
- No other source files touched.
- No public API change, no schema change, no wire change.

## Cycle commits

| SHA | Title |
|---|---|
| `80cca0d` | m9-81: vault (exploration-report + proposal + spec + tasks) |
| `a3f59ea` | m9-81: route 6 counterexample_storage read paths through table_error |
| `93f7cf5` | m9-81: verify-phase cycle-artifacts (apply-checkpoint + receipts) |
| `fdc5acc` | Merge branch 'feat/m9-81-counterexample-table-classifier' into main |
| `a4c4dd9` | m9-81: release-phase artifacts (v0.7.83, merge fdc5acc) |
| `3106fc5` | m9-81: record origin-push state in release-receipt |

## Vault delta

- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/`:
  exploration-report.md, proposal.md, spec.md, tasks.md, change-entry.md
  (5 new files).
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: m9-81 row
  appended (initially OPEN, closed in archive phase).
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: Last updated
  timestamp bumped; FIND-M9-72 closure recorded.
- `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/`:
  apply-checkpoint.json, implementation-receipt.md, verify-findings.json,
  verify-report.md, release-receipt.md, merge-receipt.md, release-report.md
  (7 cycle-artifacts).

## Cross-checks

- **CC#4** (archive-manifest SHA propagation): PASS — `python3 scripts/regen_manifest_index_shas.py` reports "nothing to do (79 manifest(s) already correct)" after archive phase.
- **CC#9** (Head SHA full 40-char): PASS — head `a3f59ea0d1bd446cd20c12f3f62863d70deb0f3a` is full 40 chars; base `45b53df132186b09de75b543b87cf0bab23bd26e` is full 40 chars.
- **CC#12** (tag peel matches main): PASS — `v0.7.83^{commit} == fdc5accf64be1fcf780913243aec0496ad48e7fe == main_sha == merge SHA`.
- **CC#21** (`## Evidence bindings` in archive-manifest): PASS — section present.
- **CC#25** (`# Change: m9-81 ...` change-entry title): PASS.
- **CC#27** (`# Release Report — m9-81-counterexample-table-classifier`): PASS.
- **CC#28** (release-receipt.md `Base SHA` field): PASS — field present.
- **CC#30** (verify-findings `verdict` field + archive-manifest `Cycle` field + change-entry `## Summary`): PASS.
- **CC#31** (archive-manifest `Base SHA` field + verify-report `Cross-checks` section): PASS.
- **CC#32** (release-report `Path` field + `## Cross-checks`): PASS.
- **CC#34** (verify-report `Cross-checks` section): PASS.
- **CC#36** (lens_summary): PASS — verify-report.md has Behaviour/Code-quality/Architectural lens summaries.
- **CC#42** (peel format full 40-char): PASS.
- **CC#49** (Base SHA full 40-char): PASS.
- **CC#51** (cycle-artifacts folder): PASS — 7 artifacts in `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/`.
- **CC#55** (Files Inventory): PASS — present in verify-report.md and archive-manifest.md.

## Carry-forward

- **Closed**: FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION.
- **New (out of scope)**: FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK — the
  `sddk cycle evaluate-gate` CLI is unable to record admission events
  on this project (FOREIGN KEY constraint + duplicate event_id), which
  forced the use of the manual vault-tracked workflow. Recommend a
  separate follow-up cycle.

## Next roadmap candidate

The m9-roadmap continues. m9+ carry-forwards still unassigned include:

- FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE (m9-75).
- FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK (new).

The M7 milestone (deferred from M6 — see `docs/ROADMAP.md`) is
larger-scope work.
