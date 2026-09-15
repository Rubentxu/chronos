# Release Receipt — m10-ms-cap-discovery-followup-2

| Field | Value |
|---|---|
| Remote tag | `v0.7.110` |
| Remote tag_peel | `e51d9e82cb14a2d47de4aec58f2edb06c81ac888` |
| Head SHA | `e51d9e82cb14a2d47de4aec58f2edb06c81ac888` |
| Base SHA | `46417ce9a224df5e2027b4553e07243a2d6795db` |
| Peel match | true |
| Date | 2026-09-15 |

## Tagging

Tag `v0.7.110` published to origin (peels to apply commit `e51d9e82`).
A-min cycle; tag at apply commit per A-lite convention
(`main_sha = head_sha = apply commit`).

## Verification

- `cargo fmt --all -- --check`: clean
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
- `cargo test -p chronos-mcp --lib --no-fail-fast`: 99/99 pass
- `cargo test -p chronos-native --lib -- --test-threads=1`: 103/103 pass
- `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e --exclude chronos-native --no-fail-fast`: all pass
