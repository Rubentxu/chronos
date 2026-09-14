# Proposal — m9-96: cc-001 god-module housekeeping

## Identification

| Field | Value |
|---|---|
| Cycle ID | `m9-96-cc001-housekeeping` |
| Workspace | `p-3416cfb8288f8964` |
| Path | A-lite (vault-only cycle; no Rust touched) |
| Status | proposed |
| Branch | `chore/m9-96-cc001-housekeeping` |
| Tag | `v0.7.98` |

## Subject

Close the **`cc-001-god-module`** finding (the only remaining
P2 MEDIUM active finding) by moving it from the "Active terms" section
to the "Terminated terms" section in
`.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`.

The finding has been fully resolved across 5 cycles:
- m9-84 — keys-split (production)
- m9-85 — impl-split (production)
- m9-86 — types-split (production)
- m9-87 — schema-split (production)
- m9-94 — storage-side test-split (1771 lines)
- m9-95 — services-side test-split (2169 lines)

After m9-96: cc-001-god-module is **fully closed**. Both files
(`counterexample_storage.rs` and `counterexample.rs`) are split into
focused sibling submodules; both test blocks live in separate
`mod tests` sibling files.

## Problem

`cc-001-god-module` was tracked since m9-04 as a P2 MEDIUM coupling
finding: `counterexample_storage.rs` at 2556 lines with 5 distinct
concerns (keys/records/persistence/schema-version/v2-legacy). The
m9-84..m9-87 cycles resolved the production halves. The m9-94 + m9-95
cycles resolved the test halves. The finding remains formally open in
`terms/index.md` "Active terms" → "Debt findings from m9-04" table.

The m9-95 closure handoff identified m9-96 as the natural next cycle
to close the finding: "The finding has been fully resolved across 5
cycles (m9-84..m9-87 + m9-94 + m9-95); updating active-findings.md
closes it. Smallest possible next cycle; sets up the m10+ backlog to
start clean."

## Approach

A-lite vault-only cycle. No Rust touched. Three changes:

1. **Move `cc-001-god-module` row** from the "Active terms" →
   "Debt findings from m9-04" table (line 36) to the "Terminated
   terms" table (after line 122).
2. **Update `findings_closed`** in the m9-96 apply-checkpoint.json
   to include `cc-001-god-module`.
3. **Standard cycle artifacts + archive + handoff** following the
   established pattern (m9-94 + m9-95 cycle structure).

After m9-96: only `cc-004-implicit-io-toctou` (P3 LOW) remains as an
active debt finding from m9-04, deferred to m10+.

## Tier

- **T0**: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` (sanity check, even though no Rust touched).
- **T1**: skipped (no Rust touched).
- **T2**: skipped (no Rust touched).
- **Vault**: `bash scripts/check_vault_drift.sh` + `python3 scripts/regen_manifest_index_shas.py --check`.

No Rust changes → no test runs required. No sandbox needed.

## Files changed

| Bucket | Files | Lines | Notes |
|---|---|---|---|
| `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | 1 modified | -1 / +1 | Move `cc-001-god-module` row from active to terminated |
| `cycle-artifacts/p-3416cfb8288f8964/m9-96-cc001-housekeeping/*.{md,json}` | 7 added | — | cycle artifacts |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-96-cc001-housekeeping/*.md` | 5 added | — | knowledge artifacts |
| `cycles/index.md` | 1 modified | +2 | m9-96 row + Total 95 → 96 |
| `terms/index.md` (Last archive update) | 1 modified | +1 | Last archive = m9-96-cc001-housekeeping |
| `apply-checkpoint.json` (root) | 1 modified | ~47 | cycle_id = m9-96-cc001-housekeeping; v0.7.98 |

**Total**: 2 source files modified + 12 cycle/knowledge artifacts + 1 index file + 1 root apply-checkpoint.

## Out-of-scope

- cc-004-implicit-io-toctou (P3 LOW; deferred to m10+).
- Any new debt findings; m9-96 only closes an existing one.

## Cross-check (CC#39 Part C)

The CC#39 Part C check counts cycle directories to verify the
`Total cycles` count in `cycles/index.md`. m9-96 must:
1. Create `cycle-artifacts/p-3416cfb8288f8964/m9-96-cc001-housekeeping/`
   (1 directory).
2. Create `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-96-cc001-housekeeping/`
   (1 directory).
3. Bump `Total cycles` from 95 → 96 in `cycles/index.md`.

This will be auto-validated by `bash scripts/check_vault_drift.sh`.

## Verification

- `bash scripts/check_vault_drift.sh`: PASS.
- `python3 scripts/regen_manifest_index_shas.py --check`: clean.
- `cargo fmt --all -- --check`: clean (no Rust touched).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean (no Rust touched).
- `cc-001-god-module` moved from active to terminated in `terms/index.md`.
- `apply-checkpoint.json` cycle_id = `m9-96-cc001-housekeeping`.
- `cycles/index.md` Total cycles = 96.
