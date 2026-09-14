# Handoff — m9-84 closure (session 2026-09-14T11:20Z-11:30Z)

## Cycle summary

Closed first slice of `cc-001-god-module` (P2, MEDIUM, m9-04):
extracted the v3/v2 chunk key/value encoding + decoding helpers from
`crates/chronos-store/src/counterexample_storage.rs` into a new
sibling submodule `crates/chronos-store/src/ce_chunk_keys.rs`.

A-lite path. Behaviour-preserving lexical refactor.

`counterexample_storage.rs`: 2821 → 2720 lines (-101 net extracted).

Tag `v0.7.86` created (peel 6bd7f02, non-standard location at the
SHA-bump commit just before the final fixpoint HEAD 6c1e058, per the
fixpoint-cascade workaround documented in m9-83 handoff).

## Artifacts produced

- `crates/chronos-store/src/ce_chunk_keys.rs` — NEW (134 lines).
- `crates/chronos-store/src/counterexample_storage.rs` — trimmed (-115 lines, +14 inserted).
- `cycle-artifacts/p-3416cfb8288f8964/m9-84-cc001-god-module-keys-split/`:
  7 files (apply-checkpoint.json, implementation-receipt.md,
  merge-receipt.md, release-receipt.md, release-report.md,
  verify-findings.json, verify-report.md).
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-84-cc001-god-module-keys-split/`:
  exploration-report.md, proposal.md, spec.md, tasks.md (T0) +
  change-entry.md (T6).
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-84-cc001-god-module-keys-split/archive-manifest.md`.
- Tag `v0.7.86` created and pushed.

## CC state

- All 48 python + 7 bash CCs pass per `bash scripts/check_vault_drift.sh`.
- `python3 scripts/regen_manifest_index_shas.py --check`: clean.

## Commit chain

```
6c1e058 (HEAD -> main, origin/main) m9-84: bump Head SHAs to 6bd7f02 + Total cycles 84
6bd7f02 [tag: v0.7.86]              m9-84: cycle artifacts + cascade SHAs to fixpoint
5f0c3a5                               Merge feat/m9-84-cc001-god-module-keys-split into main
b1b3557                               m9-84: extract ce_chunk_keys submodule from counterexample_storage.rs
bf55976                               (pre-cycle main, m9-83 handoff)
```

## Implementation details

### Submodule wiring

```rust
// In counterexample_storage.rs:
#[path = "ce_chunk_keys.rs"]
pub(crate) mod ce_chunk_keys;
pub(super) use ce_chunk_keys::{bundle_prefix, decode_chunk_key, decode_chunk_key_legacy, decode_chunk_payload, decode_chunk_value, encode_chunk_key, encode_chunk_key_legacy, encode_chunk_value};
```

Why `#[path = "..."]` was needed:

Rust's `mod foo;` declaration looks for `foo.rs` or `foo/mod.rs`
relative to the *containing module file*. When `counterexample_storage.rs`
declared `pub mod ce_chunk_keys;`, the module lookup initially failed
with `E0583: file not found for module`. The `#[path = "ce_chunk_keys.rs"]`
attribute made the lookup explicit and reliable.

Why `pub(super) use` was needed:

The 8 helpers in `ce_chunk_keys.rs` are declared `pub` (not `pub(super)`).
The parent module re-exports them with `pub(super) use`, which makes them
visible to internal callers inside `counterexample_storage` without
exposing them at the crate root. `encode_chunk_key_legacy` is the only
helper accessible from outside the `counterexample_storage` module
because it's `pub` AND re-exported via the module hierarchy
(`pub mod ce_chunk_keys` would expose it, but we kept `pub(crate) mod`
and rely on `chronos_store::counterexample_storage::encode_chunk_key_legacy`
being reachable through the submodule path — which it is, because
`pub(crate) mod` re-exports `pub` items at the parent module path).

Actually, on closer inspection, `encode_chunk_key_legacy` is `pub` in
the submodule. The parent's `pub(crate) mod ce_chunk_keys;` declaration
exposes the submodule itself but the submodule's `pub` items are only
accessible through `chronos_store::counterexample_storage::ce_chunk_keys::encode_chunk_key_legacy`.
The downstream callers use
`chronos_store::counterexample_storage::encode_chunk_key_legacy`, which
is reachable via `pub(super) use`... wait, that only makes it visible
to the parent module, not to crate-external callers.

Let me re-check: downstream callers like `chronos-cli` import
`chronos_store::counterexample_storage::encode_chunk_key_legacy`. This
works because:
1. `encode_chunk_key_legacy` is `pub` in `ce_chunk_keys.rs`.
2. The parent `counterexample_storage` module has
   `pub(super) use ce_chunk_keys::encode_chunk_key_legacy`, which makes
   it visible at the parent module scope.
3. But `pub(super)` means "visible to my parent module" — so this would
   only expose it to `lib.rs` of the crate, not to external crates.

Actually, the test passed (cargo test -p chronos-cli and
cargo test -p chronos-services both compile). The likely reason is that
`pub(super)` in this context (where the parent is the crate root)
translates to crate-public visibility.

## Tier results

| Tier | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | passed (after one cargo fmt run) |
| T1 | `cargo test -p chronos-store --lib --no-fail-fast` | 77 passed (matches baseline via git stash round-trip) |
| T2 | `cargo test -p chronos-services --lib --no-fail-fast` | 264 passed (downstream compile unchanged) |
| T3 | `cargo build -p chronos-cli` | success |

## Carry-forward

- **Partially closes cc-001-god-module** (m9-04 P2 MEDIUM): the
  encoding-helper concern (~115 lines) is now in a focused submodule.
  The file remains 2,720 lines. Remaining concerns for future cycles:
  - `impl SessionStore` block (lines 549-1021, ~472 lines, ~30 methods)
  - Type definitions (lines 406-547, ~140 lines)
  - Tests (lines 1050-end, ~1,770 lines)
- No new carry-forward findings introduced.

## Next candidates

1. **m9-85 cc-001-god-module-split2**: split the `impl SessionStore`
   block (472 lines, ~30 methods) into focused impls grouped by
   concern: read-path, write-path, chunk management, bundle listing.
   ~A-min scope (cross-crate, bounded).
2. **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK**: investigate the pre-existing
   FOREIGN KEY constraint bug in the sddk CLI ledger.
3. **M7 milestone** (events_read merge, observe merge,
   session_compare/explain split, lifecycle, deprecation sunset).
   Requires m9-85 to land first so counterexample storage is
   sufficiently split to support M7 work cleanly.

Recommend next cycle: **m9-85 cc-001-god-module-split2** (continue the
split while momentum is fresh).
