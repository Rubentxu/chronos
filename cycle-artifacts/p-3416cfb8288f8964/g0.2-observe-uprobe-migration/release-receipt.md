# Release Receipt — g0.2-observe-uprobe-migration

## Identification

| Field | Value |
|---|---|
| Cycle | g0.2-observe-uprobe-migration |
| Path | A-min, wire-contract pin (no code change) |
| Base SHA | c27765bd1acf421f9fc7a8a4cf8c4cdd824daee4 |
| Head SHA | 13495faee3c6a6ec25ea1b8dab42b06c6624858a |
| Main SHA | 13495faee3c6a6ec25ea1b8dab42b06c6624858a |
| Remote tag | v0.7.112 |
| Remote tag_peel | 0be2ec2d53d9698956ae705938b32b80d7365ad7 |

## Release result

G0.2 ships 2 typed-error tests in `tests/observe_uprobe.rs` plus
`exploration-report.md` with verbatim wire-smoke output against the
real `chronos-mcp` binary. No Rust code change required — the
`probe_inject → observe(verb="create", condition.kind="uprobe")`
migration was completed in m7-02. The cycle pins the negative
contract (discriminator snake_case, tagged-enum scope, RpcError
conversion) and confirms it against the running binary.

The cycle does NOT create a new release tag. Canonical reference
remains `v0.7.112` peel `0be2ec2d` (intact).

| Peel match | true |
