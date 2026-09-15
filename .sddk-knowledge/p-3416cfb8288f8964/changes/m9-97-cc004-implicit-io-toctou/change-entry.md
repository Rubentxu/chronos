# Change: m9-97 cc-004 implicit I/O TOCTOU falsification

## Summary

Cycle closed with no follow-up debt. Schema and drift sweep clean.

## Subject

- **Cycle**: m9-97-cc004-implicit-io-toctou
- **Path**: A-min, reclassified to evidence-backed vault-only closure
- **Branch**: `chore/m9-97-cc004-implicit-io-toctou`
- **Base SHA**: `5600873d05e0ab79f17eed6eec63df96ddd97c88`
- **Tag**: `v0.7.99`
- **Status**: completed

## Result

cc-004 was a false positive. The separate `ReadTransaction` in
`save_bundle_record_and_events` is created only after the function already
owns a redb `WriteTransaction`. redb 2.6.3 serializes writers in
`TransactionTracker::start_write_transaction()` by waiting while
`live_write_transaction` is set. A racing save cannot begin or commit in the
alleged read-to-delete window.

A local experimental Rust refactor was committed and immediately reverted
before publication after this falsification. The released tree has no net Rust
change for m9-97. cc-004 is terminated as evidence-disproved, not as a code
fix.

## Files changed

- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: move cc-004 from active
  to terminated with redb single-writer evidence.
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: record m9-97.
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-97-cc004-implicit-io-toctou/`:
  proposal, spec, tasks, exploration, and this change entry.
- `cycle-artifacts/p-3416cfb8288f8964/m9-97-cc004-implicit-io-toctou/`:
  evidence receipts, verification, release, and archive records.

## Verification

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace --lib -- --test-threads=1`: clean.
- `python3 scripts/regen_manifest_index_shas.py --check`: clean.
- `bash scripts/check_vault_drift.sh`: clean after cycle artifacts exist.

## Carry-forward

- `FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK`: external SDDK CLI issue.
- `FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL`: documented environmental
  flake, not a debt finding.
- `FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION`: non-blocking
  observation handled by the regen tool.
