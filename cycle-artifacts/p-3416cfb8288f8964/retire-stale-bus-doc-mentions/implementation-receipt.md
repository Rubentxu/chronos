# Implementation receipt — retire-stale-bus-doc-mentions

> Retroactively produced for the ledger sync. The work itself landed in
> commit `23757379` and `99c0dcee` (already on `main`).

## Commits merged to `main`

| SHA | Sub-cycle | Description |
|---|---|---|
| `23757379` | doc-only cleanup + tree hygiene | 5 doc-only edits across `crates/chronos-mcp/src/server.rs`, `crates/chronos-native/src/{probe_backend.rs,capture_runner.rs}`, `crates/chronos-log/src/memory.rs`. Plus 2 residual test stubs dropped from `chronos-sandbox/tests/m1_acceptance.rs`. Bundled pre-existing tree hygiene: `cargo fmt` drift on `probe_backend.rs` and `clippy::new_without_default` on `NativeProbeBackend` (fixed with `impl Default { Self::new() }`) per AGENTS.md §4 option A. Diff: 6 files, +24/−25. |
| `99c0dcee` | cycle-artifacts | `cycle-artifacts/p-3416cfb8288f8964/retire-stale-bus-doc-mentions/{proposal,tasks}.md` |

## Verification (per commit message)

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
  (after Default impl patch).
- `cargo test -p chronos-log -p chronos-native -p chronos-services -p chronos-mcp --lib --no-fail-fast -- --test-threads=1`:
  - chronos-log: 77/77
  - chronos-native: 107/107 (serial)
  - chronos-services: 370/370
  - chronos-mcp: 99/99 (background capture, exit 0)

## Ratchet

- `python3 scripts/check_legacy_evb.py`: PASS, baseline = 0
- `legacy-evb-inventory.json` regenerated to 0 entries.

## Acceptance check

Each doc-only edit site replaced a description of *removed* behavior
with a description of *current* behavior while preserving a reference
to REC-C2.3 for the archaeological trail. The two test stubs were dead
code (`let backend_unused_eventbus_arg_removed = ();`) flagged by
clippy as unused. No Rust semantics changed.