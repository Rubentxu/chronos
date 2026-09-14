# Tasks — m9-96: cc-001 god-module housekeeping

## Phase 1: Vault update

### T1.1: Move cc-001-god-module row from active to terminated

**File**: `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`

Remove the row from line 36:
```
| cc-001-god-module | m9-04 | coupling | MEDIUM | P2 | `counterexample_storage.rs` at 2,556 lines; 5 distinct concerns | unassigned | m9+ backlog |
```

Add a new row to the "Terminated terms" table (after line 122):
```
| cc-001-god-module | m9-04 | `counterexample_storage.rs` at 2,556 lines; 5 distinct concerns (keys/records/persistence/schema-version/v2-legacy) | m9-96-cc001-housekeeping (`v0.7.98`) |
```

The "Terminated terms" table has 4 columns: `ID`, `Cycle`, `Título`, `Closed by`. The new row follows the format of the most recent terminated rows (m9-75 row at line 123 is the closest precedent).

### T1.2: Run T0 sanity check (no Rust touched)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

**Expected**: clean (no diff; nothing changed in Rust).

## Phase 2: Cycle artifacts + archive

### T2.1: Write 7 cycle artifacts under `cycle-artifacts/p-3416cfb8288f8964/m9-96-cc001-housekeeping/`

Per AGENTS.md standard cycle artifact pattern:
- `apply-checkpoint.json` (with `findings_closed: ["cc-001-god-module"]`)
- `implementation-receipt.md`
- `merge-receipt.md`
- `release-receipt.md`
- `release-report.md` (with `## Cross-checks` section per CC#39 Part B)
- `verify-findings.json`
- `verify-report.md`

### T2.2: Write archive-manifest

**File**: `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-96-cc001-housekeeping/archive-manifest.md`

Include SHA-256 evidence bindings for all 7 cycle artifacts + 5 knowledge artifacts + 2 modified files (terms/index.md, cycles/index.md).

### T2.3: Update cycles/index.md

Add m9-96 row + bump Total 95 → 96.

### T2.4: Update terms/index.md (Last archive)

Set Last archive = m9-96-cc001-housekeeping.

### T2.5: Run `python3 scripts/regen_manifest_index_shas.py`

Cascade SHA fixpoint across all archive-manifests.

### T2.6: Run `bash scripts/check_vault_drift.sh`

Verify all CCs pass.

### T2.7: Pre-create tag v0.7.98 at cycle-artifacts commit

Per CC#42 fixpoint-cascade workaround.

### T2.8: `--no-ff` merge into main + cascade SHA bumps + push

Standard pattern: cycle artifacts commit → cascade commits (SHA-256 fixpoint) → merge → archive + handoff → push to origin.

### T2.9: Write handoff

**File**: `.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-96-cc001-housekeeping-closure-2026-09-14.md`

Include lessons learned + recommendations for next cycle.

### T2.10: Delete cycle branch

Per CC#53 hygiene.

## Verification commands

```bash
# T0 sanity (no Rust touched, but verify nothing is broken)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# Vault
python3 scripts/regen_manifest_index_shas.py --check
bash scripts/check_vault_drift.sh

# Verify no Rust source modified
git diff main..chore/m9-96-cc001-housekeeping -- crates/ Cargo.toml Cargo.lock
```
