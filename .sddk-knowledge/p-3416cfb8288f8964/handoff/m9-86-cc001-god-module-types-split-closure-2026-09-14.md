# Handoff — m9-86 closure (session 2026-09-14T12:15Z-12:28Z)

## Cycle summary

Closed the third slice of `cc-001-god-module` (P2, MEDIUM, m9-04):
extracted the 6 `pub` types + the `default_schema_version` serde helper
from `crates/chronos-store/src/counterexample_storage.rs` (lines 327-513
post-m9-85, ~187 lines) into a new sibling submodule
`crates/chronos-store/src/ce_types.rs` (165 lines, NEW).

A-min path. Behaviour-preserving lexical refactor.

`counterexample_storage.rs`: 2287 → 2156 lines (-131 net extracted).

Tag `v0.7.88` created (peel `434f2b4db74e94488f180100a8e8c8db50fd7fa3`,
non-standard location at the cycle-artifacts commit just before the
SHA-cascade fixpoint, per the fixpoint-cascade workaround documented
in m9-83 handoff).

## Artifacts produced

- `crates/chronos-store/src/ce_types.rs` — NEW (165 lines). Contains
  6 `pub` types + `pub(crate) fn default_schema_version`.
- `crates/chronos-store/src/counterexample_storage.rs` — trimmed (2287 → 2156 lines, -131).
- `cycle-artifacts/p-3416cfb8288f8964/m9-86-cc001-god-module-types-split/`:
  7 files (apply-checkpoint.json, implementation-receipt.md,
  merge-receipt.md, release-receipt.md, release-report.md,
  verify-findings.json, verify-report.md).
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-86-cc001-god-module-types-split/`:
  exploration-report.md, proposal.md, spec.md, tasks.md.
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-86-cc001-god-module-types-split/archive-manifest.md`.
- Tag `v0.7.88` created and pushed.

## CC state

- All 48 python + 7 bash CCs pass per `bash scripts/check_vault_drift.sh`.
- `python3 scripts/regen_manifest_index_shas.py --check`: clean (84 manifests).

## Commit chain

```
c520f9c (HEAD -> main, origin/main) m9-86: archive-manifest + terms bump + cascade SHAs
1bb6908                              m9-86: bump Head SHAs to 434f2b4 + Total cycles 86
434f2b4 [tag: v0.7.88]               m9-86: cycle artifacts (apply-checkpoint, implementation-receipt, merge-receipt, verify-report, verify-findings)
db2147d                              Merge feat/m9-86-cc001-god-module-types-split into main
8dcbff8                              m9-86: extract ce_types submodule from counterexample_storage.rs
43e48b9 (origin/main, origin/HEAD)   docs(handoff): append m9-85 closure (session 2026-09-14T11:35Z-11:55Z)
```

Note: the cycle-artifacts commit (`434f2b4`) is followed by two cascade
commits (`1bb6908` → `c520f9c`). The tag `v0.7.88` peels at
`434f2b4db74e94488f180100a8e8c8db50fd7fa3`, matching the CC#42
fixpoint-cascade workaround documented in m9-83 handoff.

## Implementation details

### Submodule wiring (final, m9-84 pattern applied)

```rust
// In counterexample_storage.rs:
#[path = "ce_types.rs"]
pub(crate) mod ce_types;
pub use ce_types::{
    CounterexampleBundleFilter, CounterexampleBundleRecord, CounterexampleBundleSummary,
    ExistencePredicateWire, HypothesisInputWire, MinimisedPayload,
};
#[cfg(test)]
pub(crate) use ce_types::default_schema_version;
```

### Why `pub use` worked for types (and not for methods in m9-85)

Unlike `impl SessionStore` methods (which are type-level items in m9-85),
`pub struct`/`pub enum` ARE module-level items. `pub use` re-export
applies cleanly. This is the m9-84 `ce_chunk_keys` pattern applied to
types.

### Why `#[cfg(test)] pub(crate) use ce_types::default_schema_version`

The test module at line 388 uses `use super::*;`. The parent's `pub use`
re-exports `pub` items, but `default_schema_version` is `pub(crate)` (it
shouldn't be `pub` because it's an internal serde helper, not user API).
The `#[cfg(test)] pub(crate) use` makes the helper reachable in test
context (where `pub(crate)` is sufficient since tests are inside the
crate) without exposing it as a public API.

### Serde default function resolution

`#[serde(default = "default_schema_version")]` resolves the function
name in the **struct's module scope**. Since the structs moved to
`ce_types.rs` and `default_schema_version` lives in the same file,
serde finds it directly. Verified by
`m9_02_legacy_bundle_deserializes_with_default_schema_version` test
passing (legacy bundle gets `schema_version = 3`).

## Test results

| Tier | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check` | passed |
| T0 | `cargo clippy --workspace --all-targets -- -D warnings` | passed (0 warnings) |
| T1 | `cargo test -p chronos-store --lib` | 77/77 (matched baseline) |
| T2 | `cargo test -p chronos-services --lib` | 264/264 (matched baseline) |
| T3 | `cargo test -p chronos-cli` | 35/35 (matched baseline; includes replay_integration 2/2) |
| T4 | `cargo test -p chronos-native --lib -- --test-threads=1` | 103/103 |
| Build | `cargo build --workspace` | success, no warnings |
| Build | `cargo build -p chronos-mcp` | success, no warnings (downstream consumer) |

## Carry-forward

The `cc-001-god-module` debt remains open. counterexample_storage.rs is
now 2,156 lines (was 2,821 pre-m9-84, 2,720 post-m9-84, 2,287 post-m9-85,
2,156 post-m9-86). Future cycles can address:

- **m9-87 candidate**: free functions section (`collect_v3_keys_for_bundle`,
  `collect_bundle_chunks_legacy`, `collect_bundle_chunks`,
  `KNOWN_BUNDLE_SCHEMA_VERSIONS`, ~110 lines).
- **Tests relocation** (~1,750 lines) — kept in same file as the impl
  they cover; could be moved to `tests.rs` in a future cycle.
- **Standalone pub fns** `bundle_events_or_legacy` and
  `bundle_events_count_or_legacy` (35 lines) — kept in parent since
  they reference types via the parent path.

## Next candidates

1. **m9-87 cc-001-god-module-split4**: extract free functions section
   (~110 lines), bring file below 2,050 lines.
2. **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK**: CLI ledger FK bug
   (carry-forward from m9-81 handoff).
3. **M7 milestone**: events_read merge, observe merge, session_compare/
   explain split, lifecycle.

## Lessons learned

- **`pub use` works for module-level items, NOT type-level items.** This
  is the inverse of the m9-85 lesson. Types (`pub struct`/`pub enum`)
  are module-level items and re-export cleanly. Inherent methods
  (`fn` inside `impl Foo`) are type-level and require no `pub use` —
  just `<Foo>::method_name` or `instance.method()`.
- **`#[serde(default = "...")]` resolves in the struct's module scope.**
  When moving structs to a submodule, ensure the default function is in
  the same submodule, OR use a fully-qualified path in the annotation.
- **`#[cfg(test)] pub(crate) use`** is a clean pattern for exposing
  helpers to test code without making them part of the public API.
