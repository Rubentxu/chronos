# Release Receipt — m9-86-cc001-god-module-types-split

## Identification

| Field | Value |
|---|---|
| Cycle | m9-86-cc001-god-module-types-split |
| Path | A-min |
| Branch | feat/m9-86-cc001-god-module-types-split |
| Date | 2026-09-14 |

## SHAs

| Field | Value |
|---|---|
| Base SHA | 43e48b927eebb30be5cd2c6fcffbd6dcbabcbc7d |
| Cycle HEAD (pre-merge) | 8dcbff8 |
| Merge commit | db2147d |
| Cycle-artifacts commit | 434f2b4db74e94488f180100a8e8c8db50fd7fa3 |
| Main SHA (post-cycle-artifacts, tag-peel) | 434f2b4db74e94488f180100a8e8c8db50fd7fa3 |
| Final fixpoint HEAD (after SHA cascade) | TBD |
| Remote tag | v0.7.88 |
| Remote tag_peel | 434f2b4db74e94488f180100a8e8c8db50fd7fa3 |
| Peel match | clean peel: v0.7.88 points at the cycle-artifacts commit 434f2b4 (one commit before the SHA-cascade fixpoint) |

## Release notes

- Tag `v0.7.88` created at the cycle-artifacts commit `434f2b4`.
- Behaviour-preserving lexical refactor: 6 `pub` types + `default_schema_version`
  helper extracted from `counterexample_storage.rs` into `ce_types.rs`.
- Public surface unchanged (all callers continue to use
  `chronos_store::counterexample_storage::TypeName` paths, no source
  modifications needed).
- Cross-checks satisfied:
  - apply-checkpoint.base_sha == release-receipt.base_sha
  - apply-checkpoint.remote_tag_peel == release-receipt.remote_tag_peel
  - cycles/index.md has m9-86 row + Total cycles = 86

## Verification

- `cargo test -p chronos-store --lib`: 77/77 (matches baseline).
- `cargo test -p chronos-services --lib`: 264/264.
- `cargo test -p chronos-cli --no-fail-fast`: 35/35 (lib 11 + replay_integration 2 + other 22).
- `cargo test -p chronos-native --lib -- --test-threads=1`: 103/103.
- `cargo build --workspace`: success, no warnings.
- `cargo build -p chronos-mcp`: success, no warnings (downstream consumer).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- `bash scripts/check_vault_drift.sh`: to be run post-cascade.

## Peel location rationale

Tag `v0.7.88` is placed at `434f2b4` (cycle-artifacts commit) rather than
the eventual fixpoint HEAD. This is the **CC#42 fixpoint-cascade workaround**
documented in m9-83 handoff:

- The cascade commits (regen-manifest-shas, bump-Head-SHAs) change files
  inside the vault but introduce no source code changes.
- Tagging the cascade commits would mean the tag points at a SHA whose only
  effect is metadata bookkeeping — confusing for downstream consumers.
- Tagging the cycle-artifacts commit (the last "real" commit before the
  metadata cascade) gives the tag a stable, content-bearing anchor.

The cascade commits update `apply-checkpoint.head_sha` and
`cycles/index.md` Total cycles to point at the final fixpoint HEAD.
