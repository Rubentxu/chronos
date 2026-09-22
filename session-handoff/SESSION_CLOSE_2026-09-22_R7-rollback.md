# Session Close Handoff — 2026-09-22 (R7.rollback)

> **Read this first when resuming**. Captures state, decisions, and the exact R8 work plan for tomorrow.

## State at session close

- **HEAD**: `a378d1cb` on `origin/main` (pre-R7.rollback commit; the rollback is staged in working tree, NOT yet committed)
- **Working tree (to be committed by R7.rollback)**:
  - `M docs/roadmap/STATE.md` (Baseline row retracted + R7.rollback row prepended)
  - `M docs/roadmap/JOURNAL.md` (R7.rollback row appended; 163 lines)
  - `+ session-handoff/SESSION_CLOSE_2026-09-22_R7-rollback.md` (this file)
- **Ledger**: 21v+3p+1pl honest distribution (unchanged from R6.1)
- **Tests local**: 186+184+527 PASS, arch gate PASSED, fmt clean
- **GH Actions on `a378d1cb`**: **2/5 RED** gates (CI run 35783328376 + Coverage run 35783328341)
- **Operator**: closed session for tomorrow's continuation

## What happened this session

### R7 (commit `a378d1cb`, PUSHED) — PREMATURE CLOSE
Declared "initiative COMPLETE" per AGENTS §8 based on:
- 10/10 ROADMAP chapters CLOSED
- Ledger 21v+3p+1pl honest distribution
- T1 unit tests 186+184+527 PASS
- Arch gate PASSED

**R7 failed to consult GH Actions gate state.** This was a process mistake — local T1 unit tests are NOT equivalent to the 5 GH Actions gates that are the actual integration gate per the project's CI/CD contract.

### Operator directive (2026-09-22T21:11Z)
> "ATENCION. El número de recuentos de cierres documentales y ciclos del roadmap que modificaron su estado a completado, no equivale a que todas las condiciones originales de aceptación del producto estén verificadas. Hay que revisar bien que se cumple con todos los criterios definidos para que realmente esten cerradas legalmente. Esto tambien alcanza a la deuda tecnica generada a lo largo de los ciclos de implementacion. Principal cuidado con las regresiones y codigo duplicado al plantear los cambios."

This is exactly the audit discipline R6 introduced for the ledger, applied universally: close-cycle counts ≠ literal acceptance criteria.

### R7.rollback (staged, NOT committed yet) — HONEST RETRACTION
- STATE.md: Baseline row retracted (no longer claims "initiative COMPLETE" with 2 RED gates), R7.rollback row prepended before R7 row (history preserved per AGENTS §8)
- JOURNAL.md: R7.rollback row appended
- R7 commit `a378d1cb` preserved in git (no history rewrite); R7.rollback adds additive correction

**Key principle**: the R7 declaration row stays visible as historical evidence of the premature closure. Future readers can trace what was claimed, why it was retracted, and what the actual gate state was.

## R8 work plan — exact, actionable, low-risk

### R8.1: Fix CI RED `data_rich.rs::test_get_event_returns_valid_event` (line 414)

**Symptom**: `Should have event_id` — test reads `event_id` at top level but server returns wrapped envelope.

**File**: `chronos-sandbox/tests/data_rich.rs` lines 412-419

**Current assertion pattern**:
```rust
let event_detail = client.get_event(&session_id, first_event_id).await.expect(...);
assert!(event_detail.get("event_id").is_some(), "Should have event_id");
assert!(event_detail.get("timestamp_ns").is_some(), "Should have timestamp_ns");
assert!(event_detail.get("thread_id").is_some(), "Should have thread_id");
```

**Actual server response** (v2 envelope, per `EventsReadOutput::ById` at `crates/chronos-services/src/output.rs:1686-1693`):
```json
{ "mode": "by_id", "session_id": "...", "event": { "event_id": N, "event_type": "...", "timestamp_ns": N, "thread_id": N, ... }, "provenance": {...} }
```

**Fix**: unwrap `event_detail["event"]` before reading fields:
```rust
let event = event_detail.get("event").expect("should have event");
assert!(event.get("event_id").is_some(), "Should have event_id");
assert!(event.get("timestamp_ns").is_some(), "Should have timestamp_ns");
assert!(event.get("thread_id").is_some(), "Should have thread_id");
```

### R8.2: Fix CI RED `data_rich.rs::test_inspect_causality_returns_valid_response` (line 475)

**Symptom**: `RpcError("invalid type: integer '0', expected a string")` — type mismatch.

**Root cause**: Client v2 `V2Causality.address: String` at `chronos-sandbox/src/client/tools.rs:1062` doesn't match server `CausalityReport.address: u64` at `crates/chronos-services/src/output.rs:771`.

**Decision**: server is source of truth (`u64`). Update client v2 struct to match.

**File**: `chronos-sandbox/src/client/tools.rs` line 1062

**Change**:
```rust
struct V2Causality {
    #[serde(default)]
    address: String,  // <-- CHANGE TO: u64
    mutation_count: usize,
    mutations: Vec<CausalityMutation>,
    #[serde(default)]
    note: Option<String>,
}
```

Then update `CausalityReport.address: u64` in client (it's already u64 in `CausalityReport` at `output.rs:769` — verify the client-side type matches).

### R8.3: Fix Coverage RED `probe_inject.rs` 4 tests (lines 111, 170, 209, 333)

**Symptom**: tests receive `Err(RpcError("observe: probe still starting up"))` instead of typed `probe_inject: capability: ebpf-uprobe` / `probe-starting` / `ProbeNotFound` prefix.

**Root cause**: v1 `probe_inject` was retired in C5.3.2 (REC-C5, see `crates/chronos-mcp/src/server.rs:527`); v2 `observe` dispatcher always prepends `observe:` prefix to error text (line 5024).

**Decision**: tests updated to match verified-in-tree server behavior (preferred — aligned with C5.3.2; introducing a v1 shim would be regression).

**File**: `chronos-sandbox/tests/probe_inject.rs`

**Changes**: in `assert_capability_error` function (around line 78-120), change prefix expectation:

```rust
let prefix = format!("probe_inject: capability: {}", expected_slot);  // OLD
let prefix = format!("observe: capability: {}", expected_slot);       // NEW
```

Also update the line 321-322 assertion (the "or" between ebpf-uprobe and probe-starting):
```rust
text.contains(&format!("probe_inject: capability: {}", CAP_EBPF_UPROBE))  // OLD
|| text.contains(&format!("probe_inject: capability: {}", CAP_PROBE_STARTING));  // OLD
// Both become "observe: capability: ..." prefix
```

### R8.4: Local verification

Run on chronos-sandbox workspace member (root, not in `crates/`):
```bash
cargo test --manifest-path chronos-sandbox/Cargo.toml --test data_rich --no-fail-fast
cargo test --manifest-path chronos-sandbox/Cargo.toml --test probe_inject --no-fail-fast
# Plus regression check on full workspace
cargo test --manifest-path crates/chronos-domain/Cargo.toml --lib --no-fail-fast  # 184/184
cargo test --manifest-path crates/chronos-services/Cargo.toml --lib --no-fail-fast  # 527/527
cargo test --manifest-path crates/chronos-mcp/Cargo.toml --no-fail-fast  # 186/186
```

### R8.5: Push + wait for 5/5 GREEN

```bash
git add -A
git -c user.email=chronos-orchestrator@local -c user.name='Chronos Orchestrator' commit -F /tmp/r8-commit-msg.txt
git push origin main
gh run watch --exit-status  # wait for new run to complete
```

**Acceptance gate**: 5/5 GH Actions GREEN on the new HEAD. Only then can close-declaration be re-attempted (R10 cycle, separate).

## R9 work plan (post-R8) — technical debt audit

Once 5/5 GREEN, R9 scans for accumulated debt across the 327 commits since 2026-09-19 handoff:
- Duplicate code introduced by recent commits
- Regressions in test coverage
- Doc drift in STATE.md/JOURNAL.md vs canonical references
- Ledger entries still honestly verified

## Files in this session-close package

| File | Status | Purpose |
|---|---|---|
| `docs/roadmap/STATE.md` | modified (uncommitted) | Baseline row retracted + R7.rollback row prepended |
| `docs/roadmap/JOURNAL.md` | modified (uncommitted) | R7.rollback row appended |
| `session-handoff/SESSION_CLOSE_2026-09-22_R7-rollback.md` | new (this file) | Full handoff context for next session |
| `session-handoff/CIH_H_HANDOFF_2026-09-19_close.md` | pre-existing untracked | Historical reference — these same 2 gates were `OPEN_REMOTE_GATE_PENDING` since 2026-09-19 |
| Commit `a378d1cb` | pushed (intact) | R7 premature close declaration — kept as historical evidence |
| Commit pending | (R7.rollback) | STATE.md + JOURNAL.md + this handoff doc, additive correction |

## Identity

- Project: chronos (Rust MCP server for time-travel debugging)
- Operator: rubentxu
- Session date: 2026-09-22
- Commit chain post-R6: `b2352a44` (R6.2) → `a378d1cb` (R7) → `[pending]` (R7.rollback)
- Next session: resume from R7.rollback commit; do R8 work plan exactly as documented above

## Critical reminders for next session

1. **R7 declared COMPLETE with 2/5 GH Actions RED. Don't trust it.** Check `gh run list --limit 5 --json name,conclusion,headSha` against current HEAD before any close-declaration.
2. **R7.rollback is additive correction**, not rebase. R7 row at STATE.md line 161 stays visible.
3. **R8 fixes are surgical** — server is canonical, tests need to catch up. No server regression, no re-architecture.
4. **AGENTS §8 close-criterion is strict**: "los gates obligatorios estén satisfechos" — 5/5 GH Actions GREEN is the bar, not local T1 unit tests.
5. **Composition-fidelity discipline**: thin adapters, no duplication, no re-implementation. R8 changes are test-only.
6. **MSRV = 1.75**: avoid `i64::cast_unsigned()` (1.87+), use `as u64` / `as i64` for type-cast puns. The R8.2 `address: u64` change is already canonical, no MSRV concern.
7. **Build mechanics**: `cargo --manifest-path` for sub-crate tests; `cd` does NOT persist. Commit messages with quotes/backticks → write to `/tmp/r*.txt` then `git commit -F`.

## Out-of-scope (NOT to be addressed in R8)

- Side-tracks: H1.4-B (ChronosServer extract), H1.5-B (cargo bench execution), H1.1.2 (CVE remediation), G0.x CI infra — env-locked, deferred per H1.5.
- Ledger promotions for M4B-001/OTEL-001/UI-001/M4A-001 — honestly blocked, separate operator decisions needed.
- v0.9.0 release — requires 5/5 GREEN first (R8), then R10 close-criteria re-check.
