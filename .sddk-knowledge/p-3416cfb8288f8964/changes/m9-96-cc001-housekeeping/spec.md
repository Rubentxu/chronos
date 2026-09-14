# Spec — m9-96: cc-001 god-module housekeeping

## ADDED Requirements

### REQ-m9-96-1: cc-001-god-module finding must be moved to terminated

The row for `cc-001-god-module` must be removed from the "Active
terms" → "Debt findings from m9-04" table in
`.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` and a corresponding
row must be added to the "Terminated terms" table.

**Acceptance criterion**:
```bash
$ grep "cc-001-god-module" .sddk-knowledge/p-3416cfb8288f8964/terms/index.md
| cc-001-god-module | m9-04 | coupling | MEDIUM | ...  (active row removed)
| cc-001-god-module | m9-04 | ... | m9-96-cc001-housekeeping (`v0.7.98`)  (terminated row added)
```

### REQ-m9-96-2: Terminated row must credit m9-96 + tag v0.7.98

The new terminated row must include the `Closed by` column value
`m9-96-cc001-housekeeping (`v0.7.98`)`.

**Acceptance criterion**: the terminated row in `terms/index.md` for
`cc-001-god-module` has `Closed by` = `m9-96-cc001-housekeeping (`v0.7.98`)`.

### REQ-m9-96-3: No Rust source files may be modified

m9-96 is a vault-only cycle. No files under `crates/*/src/` may be
modified. No `Cargo.toml` may be modified.

**Acceptance criterion**:
```bash
$ git diff main..chore/m9-96-cc001-housekeeping -- 'crates/' 'Cargo.toml' 'Cargo.lock'
(empty output)
```

## MODIFIED Requirements

None.

## REMOVED Requirements

None.

## Scenarios

### Scenario S1: cc-001-god-module moves to terminated

**Given**: m9-96 changes applied
**When**: `grep "cc-001-god-module" .sddk-knowledge/p-3416cfb8288f8964/terms/index.md` runs
**Then**: 1 line in the "Active terms" section is removed and 1 line is added in the "Terminated terms" section.

### Scenario S2: cycles/index.md Total cycles = 96

**Given**: m9-96 changes applied
**When**: `grep "Total cycles" .sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` runs
**Then**: `Total cycles | 96` is reported.

### Scenario S3: No Rust source modified

**Given**: m9-96 changes applied
**When**: `git diff main..chore/m9-96-cc001-housekeeping -- crates/ Cargo.toml Cargo.lock` runs
**Then**: empty output (no Rust source modified).

### Scenario S4: Vault drift sweep passes

**Given**: m9-96 changes applied
**When**: `bash scripts/check_vault_drift.sh` runs
**Then**: `vault-drift-sweep: PASS`.

## Out-of-scope

- cc-004-implicit-io-toctou (P3 LOW; deferred to m10+).
- Any new debt findings; m9-96 only closes an existing one.
- Any Rust changes.

## Acceptance criteria (summary)

1. `bash scripts/check_vault_drift.sh`: PASS.
2. `python3 scripts/regen_manifest_index_shas.py --check`: clean.
3. `cargo fmt --all -- --check`: clean.
4. `cargo clippy --workspace --all-targets -- -D warnings`: clean.
5. `cc-001-god-module` moved from active to terminated in `terms/index.md`.
6. `cycles/index.md` Total cycles = 96.
7. No Rust source files modified.
