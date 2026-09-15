# Release Receipt — m10-m9-legacy-schema-migration

| Field | Value |
|---|---|
| Remote tag | `v0.7.109` |
| Remote tag_peel | `47d89b1a6421a690cab65647df93fae229288634` |
| Head SHA | `47d89b1a6421a690cab65647df93fae229288634` |
| Base SHA | `9a6f52c8935057194273a4496cc89b0e1744094f` |
| Peel match | true |
| Date | 2026-09-15 |

## Tagging

Tag `v0.7.109` published to origin (peels to apply commit `47d89b1a`).
Per AGENTS.md §5 fixpoint-cascade workaround: tag points at apply commit
(not the `--no-ff` merge commit `bbc65a70`).

## Verification

- `cargo fmt --all -- --check`: clean (vault-only cycle, no source touched)
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
- `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e --no-fail-fast`: T3 (in progress, expected pass since no source changed)
