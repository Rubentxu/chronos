# Change: m9-82 MCP tools disclose degraded (in-memory) store mode

## Summary

Closes FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE by adding
disclosure at three layers:

1. **`chronos-store`** — a `StoreKind` enum (`Persistent | InMemory`) +
   `kind` field on `SessionStore`, exposed through
   `pub fn is_persistent(&self) -> bool`.
2. **`chronos-mcp`** — a `degraded: bool` field on `ChronosServer`, set
   once in `from_store` from `SessionStore::is_persistent()`, exposed
   through `pub fn is_degraded(&self) -> bool`.
3. **Wire layer (MCP tool envelopes)** — a `session_envelope(degraded,
   value)` helper injects a top-level `"degraded": <bool>` into the
   JSON envelope of `save_session`, `list_sessions`, `load_session`,
   `delete_session`, and `drop_session`.

The change is **wire-shape-additive**: every existing envelope field is
preserved unchanged; only one new key is added per envelope. Clients
that ignore unknown fields keep working unchanged.

## Subject

- Cycle: `m9-82-degraded-store-disclosure`
- Tag: `v0.7.84`
- Merge SHA: `b8694eff737293bffea4ba62f07e4b206eafc502`
- Path: A-min
- Tier required: T2

## Files changed

- `crates/chronos-store/src/storage.rs` — adds `StoreKind` enum, `kind`
  field on `SessionStore`, `is_persistent()` accessor; sets `kind` in
  all three constructors and the inline test struct. 3 new unit tests.
  (Net +69 lines / 73 inserts / 4 deletes.)
- `crates/chronos-mcp/src/server.rs` — adds `degraded: bool` field on
  `ChronosServer`, `is_degraded()` accessor, `session_envelope()`
  helper; wraps the success arm of all 5 session-persistence tool
  envelopes (save_session, list_sessions, load_session, delete_session,
  drop_session — both branches of the last). 5 new unit tests.
  (Net +206 lines / 217 inserts / 11 deletes.)
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-82-degraded-store-disclosure/` —
  new exploration-report.md, proposal.md, spec.md, tasks.md,
  change-entry.md (5 new files). proposal.md and spec.md carry a
  mid-impl correction note documenting the actual MCP tool names
  (save_session / list_sessions / load_session / delete_session /
  drop_session instead of the underscores-as-separators draft).
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-82-degraded-store-disclosure/archive-manifest.md` —
  new archive-manifest.md.
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` — appended m9-82 row; Total cycles 83 → 84; m9-82 row flipped OPEN → CLOSED.
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` — Last updated + Last archive bumped; FIND-M9-75 closure recorded; row removed from live "deferred from m9-75" table.
- `cycle-artifacts/p-3416cfb8288f8964/m9-82-degraded-store-disclosure/` —
  apply-checkpoint.json, implementation-receipt.md, verify-findings.json,
  verify-report.md, release-receipt.md, merge-receipt.md, release-report.md
  (7 cycle-artifacts).

## Behavioural impact

- `cargo test -p chronos-store --lib --no-fail-fast`: 74 → 77 (+3, all new is_persistent tests).
- `cargo test -p chronos-mcp --lib --no-fail-fast`: 82 → 87 (+5, all new degraded-envelope tests).
- `cargo test -p chronos-services --lib --no-fail-fast`: 264 / 0 (unchanged).
- `cargo test -p chronos-store -p chronos-mcp -p chronos-services --tests --no-fail-fast`: 477 / 0 / 0 across 10 binaries (focused T3).
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --all -- --check`: 0 diffs.
- T4-smoke (chronos-sandbox subset): e2e_connectivity 1/0, session_persistence 4/0, session_lifecycle 8/0 = **13/0**.

## Cross-check

- CC#12: `main_sha == head_sha == remote_tag_peel == b8694eff737293bffea4ba62f07e4b206eafc502`
- CC#28: `Base SHA = a0f72c2a7fe36eaeb9c772505dfe563f85f42773` in release-receipt.md
- CC#30: verify-findings.json has `verdict: "passed"`; archive-manifest.md has `## Cycle`; change-entry.md has `## Summary`
- CC#31: archive-manifest.md has `Base SHA` field; verify-report.md has `## Cross-checks` section
- CC#42: `tag_peel_sha = b8694eff737293bffea4ba62f07e4b206eafc502` (full 40-char hex)
- CC#51: cycle-artifacts folder exists with 7 artifacts
- CC#55: Files Inventory present in verify-report.md, implementation-receipt.md, and archive-manifest.md

> **Cycle**: `p-3416cfb8288f8964/m9-82-degraded-store-disclosure`
> **Tag**: `v0.7.84`
> **Merge SHA**: `b8694eff737293bffea4ba62f07e4b206eafc502`
> **Base SHA**: `a0f72c2a7fe36eaeb9c772505dfe563f85f42773`
> **Date**: 2026-09-14
