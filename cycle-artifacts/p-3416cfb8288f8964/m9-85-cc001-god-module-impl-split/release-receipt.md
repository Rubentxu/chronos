# Release Receipt — m9-85-cc001-god-module-impl-split

## Identification

| Field | Value |
|---|---|
| Cycle | m9-85-cc001-god-module-impl-split |
| Path | A-min |
| Branch | feat/m9-85-cc001-god-module-impl-split |
| Date | 2026-09-14 |

## SHAs

| Field | Value |
|---|---|
| Base SHA | 2c2a5cc8f8370eb64dca7fb4ddc47a6f3e8b15a7 |
| Cycle HEAD (pre-merge) | 158f5b1bdc4a8d33c2b34ef33b66ab8b3cf8a0bf |
| Merge commit | 72e120c |
| Cycle-artifacts commit | a850346cc3f74e07fbc19f6a95b0ff5be1d2f99e |
| Main SHA (post-cascade, fixpoint) | f94ead9 |
| Remote tag | v0.7.87 |
| Remote tag_peel | a850346cc3f74e07fbc19f6a95b0ff5be1d2f99e |
| Peel match | clean peel: v0.7.87 points at the cycle-artifacts commit a850346 (one commit before the SHA-cascade fixpoint) |

## Release notes

- Tag `v0.7.87` created at the cycle-artifacts commit `a850346`.
- Behaviour-preserving lexical refactor: 11-method `impl SessionStore` block
  split into 3 sibling submodules grouped by concern (write/read/test-hooks).
- Public surface unchanged (all callers continue to use
  `instance.method_name(...)` form, no path-qualified changes).
- Cross-checks satisfied:
  - apply-checkpoint.base_sha == release-receipt.base_sha
  - apply-checkpoint.remote_tag_peel == release-receipt.remote_tag_peel
  - cycles/index.md has m9-85 row + Total cycles = 85

## Verification

- `cargo test -p chronos-store --lib`: 77/77 (matches baseline).
- `cargo test -p chronos-services --lib`: 264/264.
- `cargo test -p chronos-cli --no-fail-fast`: 35/35 (lib 11 + replay_integration 2 + other 22).
- `cargo test -p chronos-native --lib -- --test-threads=1`: 103/103.
- `cargo build --workspace`: success, no warnings.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- `bash scripts/check_vault_drift.sh`: to be run post-cascade.

## Peel location rationale

Tag `v0.7.87` is placed at `a850346` (cycle-artifacts commit) rather than
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
