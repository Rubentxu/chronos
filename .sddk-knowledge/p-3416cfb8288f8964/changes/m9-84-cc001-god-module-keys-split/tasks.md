# Tasks — m9-84-cc001-god-module-keys-split

## Slice 1 — extract ce_chunk_keys submodule

### Task 1.1 — create ce_chunk_keys.rs

Create the new file `crates/chronos-store/src/ce_chunk_keys.rs` with
the moved content (lines 100-225 of `counterexample_storage.rs`):

- Module doc explaining the encoding scheme (v3 fixed-width, v2 legacy).
- Imports: `blake3::Hasher`, `chronos_domain::TraceEvent`.
- `pub(super) fn bundle_prefix` (was `fn`, now visible to parent module).
- `pub(super) fn encode_chunk_key`.
- `pub(super) fn decode_chunk_key`.
- `pub(super) fn encode_chunk_value`.
- `pub(super) fn decode_chunk_value`.
- `pub(super) fn decode_chunk_payload`.
- `pub fn encode_chunk_key_legacy` (stays public — visible at
  `chronos_store::counterexample_storage::encode_chunk_key_legacy`).
- `pub(super) fn decode_chunk_key_legacy`.

### Task 1.2 — declare submodule in parent

In `counterexample_storage.rs`, near the top (after the imports
section), add:

```rust
pub mod ce_chunk_keys;
```

This re-exports the submodule's items at the parent module path,
so all internal references (`bundle_prefix(bundle_id)` inside
`collect_bundle_chunks_range`, etc.) keep working without changes.

### Task 1.3 — remove moved lines

Delete lines 100-225 from `counterexample_storage.rs` (the
"m9-04 v3 key/value encoding" section, the "Legacy v2 key/value
encoding" section, and the `BUNDLE_EVENTS_CHUNK_SIZE` const stays in
the parent since it's referenced widely).

Wait — `BUNDLE_EVENTS_CHUNK_SIZE` (line 222) is referenced by
`save_bundle_record_and_events` (line 631-660 area) and tests.
Since it's only one line, keep it in the parent to minimize churn.

### Task 1.4 — verify build + tests + clippy

- `cargo build -p chronos-store` (compile check).
- `cargo test -p chronos-store --lib --no-fail-fast` (77 tests pass).
- `cargo test -p chronos-services --lib --no-fail-fast` (downstream).
- `cargo test -p chronos-cli --lib --no-fail-fast` (downstream).
- `cargo clippy --workspace --all-targets -- -D warnings` (clean).

### Task 1.5 — commit + tag + merge

- Commit on `feat/m9-84-cc001-god-module-keys-split`.
- `--no-ff` merge into main.
- Tag `v0.7.86` (clean peel match at merge commit).
- Delete cycle branch.

### Task 1.6 — cycle-artifacts + handoff

- Create `cycle-artifacts/p-3416cfb8288f8964/m9-84-cc001-god-module-keys-split/`
  with: apply-checkpoint.json, implementation-receipt.md, merge-receipt.md,
  release-receipt.md, release-report.md, verify-findings.json, verify-report.md.
- Create `.sddk-knowledge/.../changes/m9-84-cc001-god-module-keys-split/change-entry.md`.
- Create `.sddk-knowledge/.../changes/archive/m9-84-cc001-god-module-keys-split/archive-manifest.md`.
- Run `python3 scripts/regen_manifest_index_shas.py` to fixpoint.
- Run `bash scripts/check_vault_drift.sh` (must be clean).
- Append handoff to `.sddk-knowledge/.../handoff/`.

## Estimated work

- Task 1.1: ~5 min (mechanical file creation).
- Task 1.2: ~1 min.
- Task 1.3: ~2 min.
- Task 1.4: ~3 min (build + test runs).
- Task 1.5: ~5 min (commit/merge/tag/push).
- Task 1.6: ~10 min (artifacts + sweep).

Total: ~25 min wall time.

## Risk mitigation

- Behaviour preservation verified via `git stash` round-trip test
  count comparison (77 / 77).
- Public surface preserved by `pub mod ce_chunk_keys` re-export.
- All downstream crates compile via `cargo build --workspace`.
- Lint clean via `cargo clippy --workspace --all-targets -- -D warnings`.
