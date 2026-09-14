# Tasks: m9-82 MCP tools disclose degraded (in-memory) store mode

> Tasks are reviewable work units.

## T0 — Vault files on cycle branch

**Files:**
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-82-degraded-store-disclosure/exploration-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-82-degraded-store-disclosure/proposal.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-82-degraded-store-disclosure/spec.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-82-degraded-store-disclosure/tasks.md`
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-82 OPEN row)
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last updated bump)

**Commit message:**
> m9-82: vault (exploration-report + proposal + spec + tasks)

## T1 — `SessionStore::is_persistent()` accessor

**Files:**
- `crates/chronos-store/src/storage.rs` (add `StoreKind` enum, `kind`
  field, accessor; set kind in `open`, `try_open`, `in_memory`)

**Acceptance:**
- `grep -n 'pub fn is_persistent' crates/chronos-store/src/storage.rs` returns 1 match.
- `cargo test -p chronos-store --lib storage::tests::test_session_store_is_persistent` passes.

**Commit message:**
> m9-82: SessionStore exposes its kind via is_persistent()

## T2 — `ChronosServer::is_degraded()` + tool envelope `degraded` field

**Files:**
- `crates/chronos-mcp/src/server.rs` (add `degraded` field, set in
  `from_store`, expose via `is_degraded`; add `degraded` to the JSON
  envelope of `session_save` / `session_list` / `session_load`)

**Acceptance:**
- `grep -n 'pub fn is_degraded' crates/chronos-mcp/src/server.rs` returns 1 match.
- `cargo test -p chronos-mcp --lib` passes.

**Commit message:**
> m9-82: ChronosServer tracks degraded mode; tools expose it in their envelope

## T3 — Vault index update + CC sweep

**Files:**
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-82 CLOSED)
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (FIND-M9-75 closure recorded)

**Commit message:**
> m9-82: vault (cycles + terms + CC sweep)

## T4 — Verify-phase cycle-artifacts

**Files:** `cycle-artifacts/p-3416cfb8288f8964/m9-82-degraded-store-disclosure/`
containing `apply-checkpoint.json`, `implementation-receipt.md`,
`verify-findings.json`, `verify-report.md`.

**Commit message:**
> m9-82: verify-phase cycle-artifacts

## T5 — Release phase (merge + tag)

Cycle branch merged --no-ff into main → merge SHA recorded. Tag
`v0.7.84` on the merge commit. Receipts:
`release-receipt.md`, `merge-receipt.md`, `release-report.md`.

**Commit message:**
> m9-82: release-phase artifacts (v0.7.84)

## T6 — Archive phase

`archive-manifest.md` + `change-entry.md`. Cycle branch deleted.

**Commit message:**
> m9-82: archive phase (archive-manifest + change-entry)

## T7 — Apply-checkpoint status flip

**File:** `cycle-artifacts/.../apply-checkpoint.json` — `status: archived`,
`archive_status: complete`, all phases complete.

**Commit message:**
> m9-82: apply-checkpoint status=archived + phases complete

## T8 — Handoff persistence

**File:** `.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-backlog-blocked-2026-09-12.md` —
append new session section.

**Commit message:**
> docs(handoff): append m9-82 closure
