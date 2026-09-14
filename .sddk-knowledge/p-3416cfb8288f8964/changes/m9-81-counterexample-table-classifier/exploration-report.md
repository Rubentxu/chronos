# Exploration Report — m9-81-counterexample-table-classifier

> **Cycle**: `p-3416cfb8288f8964/m9-81-counterexample-table-classifier`
> **Status**: explore-complete; spec/apply/release/archive pending
> **Date explored**: 2026-09-14
> **Branch**: `feat/m9-81-counterexample-table-classifier` (from `45b53df` == `origin/main`)

## Subject

| Base | Head (current) | CWD | Verified at |
|---|---|---|---|
| `45b53df132186b09de75b543b87cf0bab23bd26e` | `45b53df132186b09de75b543b87cf0bab23bd26e` (no work yet) | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-14T09:12Z |

## Problem statement (carry-forward FIND)

`.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` lists:

> **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** | m9-72 |
> `counterexample_storage.rs` keeps four hand-rolled copies of the read-path
> `TableDoesNotExist` / else-propagate policy that `chronos-store::table_error`
> now names | unassigned | m9+

m9-72 created `crates/chronos-store/src/table_error.rs` (canonical helper:
`classify_read_table_error` + `or_not_found` / `session_not_found`) and applied
it to `storage.rs` and `cas.rs`. `counterexample_storage.rs` was missed.

## Findings (recon)

### F1 — 6 inline sites in counterexample_storage.rs duplicate the policy

`crates/chronos-store/src/counterexample_storage.rs` contains 6 read-path
sites that hand-roll the same `Ok(t) | TableDoesNotExist -> Ok(empty) |
else -> Err(Database)` ladder that `table_error::classify_read_table_error`
now names:

| # | Line | Function | Table | "Absent" answer |
|---|---|---|---|---|
| 1 | 237 | `collect_bundle_chunks_range` | `COUNTEREXAMPLE_BUNDLE_EVENTS` | `Ok(Vec::new())` |
| 2 | 286 | `collect_v3_keys_for_bundle` | `COUNTEREXAMPLE_BUNDLE_EVENTS` | `Ok(Vec::new())` |
| 3 | 326 | `collect_bundle_chunks_legacy` | `COUNTEREXAMPLE_BUNDLE_EVENTS` | `Ok(Vec::new())` |
| 4 | 697 | `get_bundle_events_count` | `COUNTEREXAMPLE_BUNDLES` | `Ok(0_u64)` |
| 5 | 875 | `load_counterexample_bundle` | `COUNTEREXAMPLE_BUNDLES` | `Ok(None)` |
| 6 | 950 | `list_counterexample_bundles` | `COUNTEREXAMPLE_BUNDLES` | `Ok(Vec::new())` |

Each site is 4 lines:
```rust
let table = match tx.open_table(FOO) {
    Ok(t) => t,
    Err(redb::TableError::TableDoesNotExist(_)) => return Ok(EMPTY),
    Err(e) => return Err(StoreError::Database(e.into())),
};
```

The canonical helper `table_error::classify_read_table_error` reduces this
to 1 line per site:
```rust
let table = match tx.open_table(FOO) {
    Ok(t) => t,
    Err(e) => return classify_read_table_error(e).or_not_found(EMPTY),
};
```

### F2 — `chronos-store::table_error` matches the pattern exactly

`crates/chronos-store/src/table_error.rs` (62 lines + 4 unit tests) was
introduced in m9-72 and already serves `cas.rs` and `storage.rs` with the
same shape:

- `crates/chronos-store/src/cas.rs:161` — `Err(e) => return classify_read_table_error(e).or_not_found(None)`
- `crates/chronos-store/src/cas.rs:191` — `Err(e) => return classify_read_table_error(e).or_not_found(false)`
- `crates/chronos-store/src/storage.rs:237` — `Err(e) => return Err(classify_read_table_error(e).session_not_found(session_id))`
- `crates/chronos-store/src/storage.rs:258` — same as 237

`or_not_found::<T>(not_found: T) -> Result<T, StoreError>` takes any "absent"
value. The refactor in `counterexample_storage.rs` differs only in the
specific absent values (`Vec::new()`, `0_u64`, `None`, `Vec::new()`,
`Vec::new()`) — all of which fit the `or_not_found` API.

### F3 — Test code is out of scope

The `#[cfg(test)] mod tests` block at line 1505+ uses
`tx.open_table(...).unwrap()` instead of the inline ladder — those are
deliberate panics in a controlled test environment, not production
behaviour. They are not in FIND-M9-72 scope and will be left alone.

### F4 — Module-level impact

- Touched source files: `crates/chronos-store/src/counterexample_storage.rs`
  (6 sites, 24 lines net removed after the refactor)
- New `use crate::table_error::classify_read_table_error;` import (1 line)
- Net diff: ~30 lines (24 removed + 1 added import + 5 churned comments)
- No public API change, no schema change, no wire change.

### F5 — Test plan

The 6 sites are all exercised by the existing `chronos-store` integration
tests. m9-72 already proved the refactor preserves behaviour in `cas.rs` /
`storage.rs` — same helper, same shape. The unit tests at
`counterexample_storage.rs` (~700 lines of `#[cfg(test)]`) cover:

- `collect_bundle_chunks_range` (range scan path) → covered
- `collect_v3_keys_for_bundle` (D6 cleanup block) → covered
- `collect_bundle_chunks_legacy` (v2 fallback) → covered
- `get_bundle_events_count` (D7 guard) → covered
- `load_counterexample_bundle` (idempotent NotFound) → covered
- `list_counterexample_bundles` (page iteration) → covered

Re-running `cargo test -p chronos-store` should be sufficient to prove
behavioural preservation.

### F6 — Risk profile

- **Mechanically equivalent refactor** — same Rust code, different shape.
  The compiler will accept only valid substitutions (the helper has the
  same input/output types as the inline ladder).
- **Single-crate** — only `chronos-store` is touched. No downstream
  crates depend on the internal shape of the read paths.
- **No wire/protocol change** — only the implementation detail of how
  "absent table" is mapped to "Ok(empty)" changes.
- **Reversible** — `git revert` restores the original file in one
  command.

## Path decision

**B-direct** — trivial, single crate, no probe/mcp touched, no
architectural fork. Per AGENTS.md tier table, this needs T0 + T1 only.

| Gate | Command | Rationale |
|---|---|---|
| T0 | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | Lint gate before tests |
| T1 | `cargo test -p chronos-store --lib --no-fail-fast` | Re-run existing 6 read-path tests |

## Carry-forward

- FIND-M9-72 closes when this cycle lands.
- No new FINDs introduced.
- No new tests introduced (existing tests cover the refactored paths).
- No new CCs (CC#9, CC#49, CC#55 already cover the artifacts we'll
  generate; CC#4 regen is a side effect of any cycles/index.md touch,
  but the cycle's own index row will be appended in archive phase).

## Next roadmap candidate after this cycle

m9+ carry-forwards still unassigned:

- **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE** — degraded
  in-memory mode not surfaced in tool responses.
- **M7 candidates** (deferred from M6) — events_read merge, observe
  merge, session_compare+session_explain split, session_start/stop
  lifecycle, deprecation sunset sweep. These are milestone-level
  (different from cycle-level m9-* debt cleanup).

## Metadata

- cycle_id: p-3416cfb8288f8964/m9-81-counterexample-table-classifier
- base_sha: 45b53df132186b09de75b543b87cf0bab23bd26e
- head_sha (current): 45b53df132186b09de75b543b87cf0bab23bd26e
- path: B-direct
- tier: T1
