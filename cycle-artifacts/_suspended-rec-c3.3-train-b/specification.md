# Specification: REC-C3.3.3 (Tren B) — Native probe MCP tool surface + service-port expansion

> Cycle: `p-3416cfb8288f8964/rec-c3.3-train-b`
> Path: **A-min** (operator-set 2026-09-18)
> Phase: **Spec**
> Author: sddk-spec (MiniMax-M3)
> Evidence base:
> - `cycle-artifacts/p-3416cfb8288f8964/rec-c3.3-train-b/exploration-report.md` (sha256 `0aaae8777ff0ad6e90b9c42b9d09eda4d450b2b4a7c5fcfec334bc36b920b9b1`)
> - `cycle-artifacts/p-3416cfb8288f8964/rec-c3.3-train-b/proposal.md` (sha256 `3a3078d569ec2e5eb5bc3e392ce2c24f33158b392e74be385e8482b4b5b9167d`)
> Constraints source: `apply-checkpoint.json::scope_decisions.constraints` (B1..B9, operator-locked 2026-09-18)

## Purpose

Specify observable behavior for the Tren B deliverables: three new MCP tools (`capture_session`, `probe_advance`, `probe_step`), two new domain ports (`SessionArchive`, `CounterexampleRepository`), a mechanical `SessionMetadata` lift to `chronos-domain`, and composition-root factory wiring — without modifying `chronos-log` (B6) and without introducing `NativeProbeServicePort` (B1). Each requirement is one observable behavior with Given/When/Then scenarios, edge cases, and slice traceability for the upcoming `sddk-tasks` phase.

## Slice Index (traceability anchor)

| Slice | Scope | REQ IDs |
|---|---|---|
| **A — SessionMetadata lift** | Move `SessionMetadata` to `chronos-domain`; re-export from `chronos-store`; swap 17 `chronos-services` call-sites. | REQ-TB-14, REQ-TB-15 |
| **B — Sandbox harness first slice** | Produce `chronos-sandbox/tests/native_probe_tools.rs`; populate `in_memory_*` composition-root test variants. | REQ-TB-16, REQ-TB-17 |
| **C — SessionArchive port + composition** | `SessionArchive` trait, `InMemorySessionArchive`, `default_session_archive()` factory. | REQ-TB-08, REQ-TB-09, REQ-TB-10 |
| **D — CounterexampleRepository port + composition** | `CounterexampleRepository` trait, `InMemoryCounterexampleRepository`, `default_counterexample_repository()` factory. | REQ-TB-11, REQ-TB-12, REQ-TB-13 |
| **E — Native ABI + service layer** | `NativeProbeBackend::advance/step`; `ProbeService::advance/step`; rewire session/counterexample services to new ports. | REQ-TB-04, REQ-TB-05, REQ-TB-06, REQ-TB-07 (partial: native + service plumbing) |
| **F — `capture_session` MCP tool** | New MCP handler composing probe lifecycle through `Arc<dyn SessionArchive>`. | REQ-TB-01, REQ-TB-02, REQ-TB-03 |
| **G — `probe_advance` + `probe_step` MCP tools** | New MCP handlers completing the wiring of `ProbeService::advance/step`. | REQ-TB-04, REQ-TB-05, REQ-TB-06, REQ-TB-07 (final: MCP surface) |

## Requirements

### REQ-TB-01: `capture_session` SHALL persist a captured session via `Arc<dyn SessionArchive>`

The `capture_session` MCP tool MUST orchestrate a single-shot capture (start probe, wait for exit / stop condition, stop probe) and persist the resulting metadata plus event stream through the composition-root-injected `Arc<dyn SessionArchive>`.

#### Scenario: capture_session success path

- GIVEN a fresh `chronos-mcp` server with the default composition-root archive factory wired
- AND a controlled target binary that exits deterministically after emitting ≥1 `TraceEvent`
- WHEN the client invokes `capture_session` with that target's program path
- THEN the tool MUST return a session id and a `saved: true` acknowledgement
- AND a subsequent `load_session` call against that session id MUST return the same metadata fields and event count
- AND the persistence step MUST have invoked `Arc<dyn SessionArchive>::save` exactly once

#### Scenario: capture_session with target that exits non-zero

- GIVEN a controlled target binary that exits with code `7` and emits ≥3 events
- WHEN the client invokes `capture_session`
- THEN the tool MUST still persist the session (non-zero exit is not a failure of capture)
- AND `SessionMetadata::end_reason` MUST record `Exited(7)`

### REQ-TB-02: `capture_session` SHALL fail closed when replay integrity is violated

If the persistence path encounters a `ReplayIntegrityError`, the tool MUST surface the variant in the wire response and MUST NOT silently swallow the error.

#### Scenario: capture_session replay integrity failure on load

- GIVEN an existing session whose backing segmented log has a `CorruptSegment` header (reproducible via `RecCorruptSegmentFixture`)
- WHEN the client invokes `capture_session` with `reopen_session_id` referencing that session
- THEN the tool MUST return `error.code = "replay_integrity_error"`
- AND the error payload MUST name the specific `ReplayIntegrityError` variant (`CorruptSegment`, `HeaderRangeMismatch`, `FilenameHeaderMismatch`, `PayloadRangeMismatch`, `RecordSessionMismatch`, `EntryCountMismatch`, `MissingRange`, `OverlappingSegments`, or `EmptySegment`)
- AND the tool MUST NOT have modified the underlying log file

### REQ-TB-03: `capture_session`'s load path SHALL go through `register_takeover`

The load step of `capture_session` MUST route through the existing `SegmentedExecutionLog::register_takeover` path that REC-C1.5.2 production code uses. This is the **fail-closed** invariant enforced by `ReplayPlan` validation.

#### Scenario: capture_session reopen goes through takeover

- GIVEN a sealed session on disk
- WHEN the client invokes `capture_session` with `reopen_session_id` referencing that session
- THEN the call stack MUST contain `SegmentedExecutionLog::register_takeover`
- AND `ReplayPlan::build_replay_plan` MUST have been invoked
- AND a sandbox assertion MUST confirm that mutating the segmented log header on disk before the reopen still surfaces the exact variant from `ReplayIntegrityError`

### REQ-TB-04: `probe_advance` SHALL resume a paused native probe

The `probe_advance` MCP tool MUST continue the target past its paused state by delegating to `NativeProbeBackend::advance`, which calls `PtraceTracer::continue_execution`.

#### Scenario: probe_advance from paused state

- GIVEN a live probe whose target is paused (e.g., after a tripwire hit or `probe_inject` breakpoint)
- WHEN the client invokes `probe_advance` with that `session_id`
- THEN the tool MUST return `advanced: true` and a new paused reason if the target re-pauses, or `advanced: true, running: true` if execution continues
- AND the underlying `PtraceTracer::continue_execution` MUST have been invoked exactly once

#### Scenario: probe_advance from running state (no-op success)

- GIVEN a live probe whose target is already running (not paused)
- WHEN the client invokes `probe_advance` with that `session_id`
- THEN the tool MUST return a no-op success acknowledgement (idempotent resume)
- AND MUST NOT raise `SessionNotPaused` as an error

### REQ-TB-05: `probe_advance` SHALL refuse to operate on a stopped or missing session

The tool MUST distinguish a non-existent session id from a stopped session, and MUST refuse to advance a stopped session.

#### Scenario: probe_advance against missing session

- GIVEN no live probe with the requested `session_id`
- WHEN the client invokes `probe_advance`
- THEN the tool MUST return `error.code = "session_not_found"`

#### Scenario: probe_advance against stopped session

- GIVEN a session that was previously stopped via `probe_stop`
- WHEN the client invokes `probe_advance`
- THEN the tool MUST return `error.code = "session_stopped"`

### REQ-TB-06: `probe_step` SHALL single-step a live native probe

The `probe_step` MCP tool MUST execute one instruction on the target by delegating to `NativeProbeBackend::step`, which calls `PtraceTracer::step`.

#### Scenario: probe_step single step

- GIVEN a live probe whose target is paused
- WHEN the client invokes `probe_step` with that `session_id`
- THEN the tool MUST return `stepped: true` and the target MUST remain paused
- AND the underlying `PtraceTracer::step` MUST have been invoked exactly once
- AND the program counter MUST have advanced by exactly one instruction (sandbox verifies against fixture binary)

#### Scenario: probe_step repeated calls

- GIVEN a paused target
- WHEN the client invokes `probe_step` 5 times sequentially
- THEN each call MUST succeed and the target MUST remain paused after each step
- AND the program counter MUST have advanced by 5 instructions

### REQ-TB-07: `probe_step` SHALL refuse to operate on a stopped or missing session

Same classification semantics as REQ-TB-05 — the tool MUST distinguish missing vs stopped vs running.

#### Scenario: probe_step against running session

- GIVEN a live probe whose target is currently running (not paused)
- WHEN the client invokes `probe_step`
- THEN the tool MUST return `error.code = "session_running"` (single-step requires paused)

#### Scenario: probe_step against missing session

- GIVEN no live probe with the requested `session_id`
- WHEN the client invokes `probe_step`
- THEN the tool MUST return `error.code = "session_not_found"`

### REQ-TB-08: `SessionArchive::save` SHALL persist `(metadata, events)` and return the session id

The `SessionArchive` port MUST expose `save(metadata: &SessionMetadata, events: &[TraceEvent]) -> Result<SessionId, SessionArchiveError>`.

#### Scenario: SessionArchive save roundtrip

- GIVEN a composition-root `Arc<dyn SessionArchive>` and a non-empty `Vec<TraceEvent>`
- WHEN the caller invokes `save` twice with distinct metadata
- THEN each call MUST persist under a distinct `SessionId`
- AND a subsequent `load` for either id MUST return metadata field-for-field identical (REQs REQ-TB-14 / REQ-TB-15 cover field identity) and the same event count

#### Scenario: SessionArchive save with empty events

- GIVEN an empty `events: Vec<TraceEvent>` slice
- WHEN the caller invokes `save`
- THEN the port MUST persist the metadata with `event_count = 0` and return the new `SessionId`

### REQ-TB-09: `SessionArchive::load` SHALL return metadata + events or a domain error

The port MUST expose `load(session_id) -> Result<(SessionMetadata, Vec<TraceEvent>), SessionArchiveError>`.

#### Scenario: SessionArchive load existing session

- GIVEN a previously saved session
- WHEN the caller invokes `load`
- THEN the port MUST return the original `SessionMetadata` and the full event vector

#### Scenario: SessionArchive load missing session

- GIVEN no persisted session with the requested id
- WHEN the caller invokes `load`
- THEN the port MUST return `SessionArchiveError::SessionNotFound`

### REQ-TB-10: `SessionArchive::list` and `delete` SHALL enumerate and remove persisted sessions; `SessionRepository` stays lifecycle-only

The port MUST expose `list() -> Vec<SessionMetadata>` and `delete(session_id) -> Result<(), SessionArchiveError>`. `SessionRepository` MUST remain unchanged in shape (lifecycle / registry only); no `save` / `load` method on `SessionRepository`.

#### Scenario: SessionArchive list returns persisted sessions

- GIVEN three previously saved sessions
- WHEN the caller invokes `list`
- THEN the port MUST return all three metadata records in a deterministic order (insertion order)

#### Scenario: SessionArchive delete removes persisted session

- GIVEN a previously saved session
- WHEN the caller invokes `delete`
- THEN a subsequent `load` MUST return `SessionArchiveError::SessionNotFound`
- AND `list` MUST NOT include that session id

#### Scenario: SessionRepository remains lifecycle-only

- GIVEN the new `SessionArchive` port
- WHEN the orchestrator inspects the `SessionRepository` trait
- THEN it MUST NOT declare `save`, `load`, `list_sessions`, or `delete_session` methods
- AND the only methods remain the existing lifecycle/registry surface (`get`, `upsert`, `remove`, `list`, `len`, `is_empty`)

### REQ-TB-11: `CounterexampleRepository::save_bundle` SHALL persist a bundle

The `CounterexampleRepository` port MUST expose `save_bundle(bundle_id: &BundleId, events: &[TraceEvent]) -> Result<(), CounterexampleRepositoryError>`.

#### Scenario: CounterexampleRepository save_bundle

- GIVEN a composition-root `Arc<dyn CounterexampleRepository>` and a non-empty event slice
- WHEN the caller invokes `save_bundle`
- THEN the port MUST persist under the supplied bundle id
- AND a subsequent `load_bundle` MUST return the same event count

### REQ-TB-12: `CounterexampleRepository::load_bundle` SHALL return events or a domain error

The port MUST expose `load_bundle(bundle_id) -> Result<Vec<TraceEvent>, CounterexampleRepositoryError>`.

#### Scenario: CounterexampleRepository load_bundle existing

- GIVEN a previously saved bundle
- WHEN the caller invokes `load_bundle`
- THEN the port MUST return the full event vector

#### Scenario: CounterexampleRepository load_bundle missing

- GIVEN no persisted bundle with the requested id
- WHEN the caller invokes `load_bundle`
- THEN the port MUST return `CounterexampleRepositoryError::BundleNotFound`

### REQ-TB-13: `CounterexampleRepository::list_bundles` SHALL enumerate bundles respecting the filter

The port MUST expose `list_bundles(filter: &CounterexampleBundleFilter) -> Vec<CounterexampleBundleSummary>` and `count_bundle_events(bundle_id) -> Result<usize, CounterexampleRepositoryError>`. The `CounterexampleBundleFilter` type MUST live in `chronos-domain::ports::counterexample` (lifted from `chronos-store`).

#### Scenario: list_bundles with empty filter returns all

- GIVEN three previously saved bundles
- WHEN the caller invokes `list_bundles(&CounterexampleBundleFilter::default())`
- THEN the port MUST return summaries for all three bundles

#### Scenario: list_bundles with restrictive filter

- GIVEN a filter that selects only bundles with `kind = Shrink`
- WHEN the caller invokes `list_bundles`
- THEN the port MUST return only those summaries whose underlying kind matches

#### Scenario: count_bundle_events matches load_bundle length

- GIVEN a previously saved bundle
- WHEN the caller invokes both `count_bundle_events` and `load_bundle`
- THEN the two counts MUST agree

### REQ-TB-14: `SessionMetadata` SHALL be lifted from `chronos-store` to `chronos-domain` with byte-for-byte field identity

The type currently at `crates/chronos-store/src/storage.rs:22` MUST move to `chronos-domain::session::SessionMetadata` (or equivalent domain module). The fields, order, derives, and serde representations MUST be byte-for-byte identical so that bincode (storage) and JSON (wire) roundtrips without drift. `chronos-store::SessionMetadata` MUST remain available via `pub use` for back-compat at the dependency boundary (mirrors C3.3.1 `SessionId` lift).

#### Scenario: SessionMetadata field identity

- GIVEN a `SessionMetadata` value constructed via the lifted type at `chronos_domain::SessionMetadata`
- WHEN the value is serialized to bincode (storage path) and then deserialized
- THEN the resulting `SessionMetadata` MUST have every field equal to the original (including `session_id`, `created_at`, `language`, `target`, `event_count`, `duration_ms`, `tail_sealed`, `sealed_at`)
- AND the byte length MUST equal the bincode length of the pre-lift type's encoding (proves no derives changed)

#### Scenario: SessionMetadata JSON wire identity

- GIVEN the same `SessionMetadata` value
- WHEN serialized to JSON via `serde_json::to_value` and round-tripped through `serde_json::from_value`
- THEN the resulting `SessionMetadata` MUST be field-for-field equal
- AND the JSON shape MUST be byte-for-byte identical to the pre-lift JSON shape (verified by snapshot of `serde_json::to_string` output)

### REQ-TB-15: All 17 `chronos-services` call-sites SHALL import `SessionMetadata` from `chronos_domain`

Every call site in `crates/chronos-services/src/` that previously imported `chronos_store::SessionMetadata` MUST import `chronos_domain::SessionMetadata` instead. Verified by `grep -rn "chronos_store::SessionMetadata" crates/chronos-services` returning zero hits.

#### Scenario: import swap completeness

- GIVEN the lift commit landed
- WHEN the orchestrator runs `grep -rn "chronos_store::SessionMetadata" crates/chronos-services`
- THEN the command MUST return no matches
- AND `cargo build -p chronos-services` MUST succeed

#### Scenario: existing ratchet tests pass post-lift

- GIVEN `chronos-sandbox/tests/session_persistence_extended.rs::save_load_roundtrip` and the m7-04 `tail_sealed` / `sealed_at` field identity checks
- WHEN the orchestrator runs them against the post-lift code
- THEN they MUST pass without modification (proves the lift was non-behavioral)

### REQ-TB-16: `chronos-mcp::composition` SHALL provide bootstrap-scoped archive factories

The composition root SHALL add four new functions: `default_session_archive() -> Arc<dyn SessionArchive>`, `default_counterexample_repository() -> Arc<dyn CounterexampleRepository>`, and `in_memory_*` test variants. These MUST be bootstrap-scoped (process-wide) because the underlying `SessionStore` is owned by `ChronosServer`.

#### Scenario: default_session_archive returns Arc over the concrete SessionStore-backed impl

- GIVEN a composition root built with default factories
- WHEN the orchestrator invokes `default_session_archive()`
- THEN it MUST return an `Arc<dyn SessionArchive>` whose concrete type wraps the existing `SessionStore::save_session` / `load_session` / `list_sessions` / `delete_session` methods
- AND `save` then `load` MUST round-trip a sample metadata + event vector

#### Scenario: in_memory_session_archive is suitable for tests

- GIVEN `in_memory_session_archive()` invoked in a unit test
- WHEN the test performs `save` / `load` / `list` / `delete` operations
- THEN they MUST all operate on in-process state (no redb file), so the test does not require a filesystem fixture

#### Scenario: default_counterexample_repository roundtrips bundles

- GIVEN a composition root built with default factories
- WHEN the orchestrator invokes `default_counterexample_repository()`
- THEN it MUST return an `Arc<dyn CounterexampleRepository>` whose concrete type wraps the existing `SessionStore::{save_counterexample_bundle, load_counterexample_bundle, list_counterexample_bundles, count_counterexample_bundle_events, load_counterexample_bundle_events}` methods
- AND a `save_bundle` then `load_bundle` MUST round-trip the same event count

### REQ-TB-17: Composition-root split between bootstrap-scoped and session-scoped SHALL be explicit

The composition-root SHALL distinguish bootstrap-scoped factories (process-wide singletons: archive factories and existing `default_execution_log_factory`, `default_uprobe_injector`, `default_browser_probe_factory`, `default_store_path`, `allow_in_memory_fallback`, `open_session_store_at`) from session-scoped concerns. `probe_advance` / `probe_step` MUST NOT require new session-scoped factories because they route through the existing `ProbeService` which receives `&ProbeContext<'_>` per call.

#### Scenario: composition-root bootstrap vs session split is documented

- GIVEN the Tren B composition-root extensions
- WHEN the orchestrator inspects `crates/chronos-mcp/src/composition.rs`
- THEN bootstrap-scoped factories MUST be grouped together (existing pattern: lines 51, 71, 89, 97, 114, 127) with new archive factories appended
- AND the file's module-level invariant comment ("the only place in `chronos-mcp` where `::new()` / `try_open()` on infrastructure types is called") MUST remain accurate

#### Scenario: probe_advance and probe_step use no new session-scoped factory

- GIVEN the new MCP tools land
- WHEN the orchestrator inspects `crates/chronos-mcp/src/composition.rs`
- THEN there MUST be no `default_native_capture_runner_factory` or analogous session-scoped factory introduced for `advance` / `step`
- AND the call sites for the two new tools MUST consume `&ProbeContext<'_>` from the existing `ProbeService` invocation chain

#### Scenario: composition-root tests cover new factories

- GIVEN the four new factory functions land
- WHEN the orchestrator inspects the in-module `composition_tests` block
- THEN there MUST be at least 4 new test cases (one per factory function) and they MUST pass under T1 (`cargo test -p chronos-mcp --lib`)

## Edge Cases

These boundaries MUST be exercised by the test plan produced in `sddk-tasks`. Each edge case maps to one or more REQs.

| ID | Edge case | Maps to REQ(s) | Test surface |
|---|---|---|---|
| **EC-TB-01** | Empty session: `capture_session` against a target that emits zero events | REQ-TB-01, REQ-TB-08 | `native_probe_tools.rs` + `session_persistence_extended.rs::save_load_roundtrip` |
| **EC-TB-02** | Paused session without target attached (race between `probe_inject` and `probe_stop`) | REQ-TB-04, REQ-TB-05 | `native_probe_tools.rs` race scenario |
| **EC-TB-03** | Replay path through `SessionArchive::load` with a tampered segmented log header | REQ-TB-02, REQ-TB-03, REQ-TB-09 | `session_persistence_extended.rs` (existing replay integrity fixture) |
| **EC-TB-04** | `probe_advance` against a session that became stopped mid-call | REQ-TB-04, REQ-TB-05 | `native_probe_tools.rs` |
| **EC-TB-05** | `probe_step` against a session whose target was reaped between lookup and step | REQ-TB-06, REQ-TB-07 | `native_probe_tools.rs` |
| **EC-TB-06** | `SessionArchive::save` called twice with identical metadata fields but different event vectors | REQ-TB-08, REQ-TB-14 | `composition_tests` |
| **EC-TB-07** | `SessionArchive::delete` against an id that does not exist | REQ-TB-10 | `composition_tests` |
| **EC-TB-08** | `CounterexampleRepository::list_bundles` with an empty result set | REQ-TB-13 | `counterexample_tools.rs` |
| **EC-TB-09** | `CounterexampleRepository::count_bundle_events` against a missing bundle | REQ-TB-13 | `counterexample_tools.rs` |
| **EC-TB-10** | `SessionMetadata` JSON shape stability across the lift (snapshot test) | REQ-TB-14 | `session_persistence_extended.rs` |
| **EC-TB-11** | Composition-root test with `allow_in_memory_fallback = true` (CI mode) | REQ-TB-16 | `composition_tests` |
| **EC-TB-12** | Concurrent `capture_session` invocations on the same composition root (race for the singleton store) | REQ-TB-01, REQ-TB-16 | `native_probe_tools.rs` + sandbox race harness |
| **EC-TB-13** | `probe_advance` idempotency: second invocation against an already-running target | REQ-TB-04 | `native_probe_tools.rs` |
| **EC-TB-14** | `probe_step` invoked against a target that hits a breakpoint mid-step | REQ-TB-06 | `native_probe_tools.rs` |

## Anti-Patterns (per B1..B9, operator-locked 2026-09-18)

Each anti-pattern is a forbidden construct. Apply phases MUST verify none appear in the diff.

| ID | Rule | Forbidden construct | Verification |
|---|---|---|---|
| **AP-B1** | B1: Reuse existing service-port abstractions; no `NativeProbeServicePort` | (a) Any new trait named `NativeProbeServicePort` or analogous aggregate service port; (b) any `services → native` edge that bypasses `LiveProbeSession` + `ProbeContext`; (c) any new `dyn ProbeService` style abstraction | `grep -rn "NativeProbeServicePort\|dyn ProbeService" crates/chronos-{domain,services,mcp}` returns no hits in the diff |
| **AP-B2a** | B2: No `dyn Any` / downcast / `chronos_store` DTO across the port | (a) `use std::any::Any` or `as_any` / `downcast_ref` inside the new ports' signatures; (b) any function parameter or return type that is a concrete `chronos_store::*` type, helper, or DTO | `grep -rn "as_any\|downcast_ref\|: Box<dyn Any>\|: dyn Any" crates/chronos-domain/src/ports/{session,counterexample}.rs` returns no hits |
| **AP-B2b** | B2: `SessionRepository` stays lifecycle / registry only | Adding `save`, `load`, `list_sessions`, `delete_session`, or any content method to `SessionRepository` | `git diff main..HEAD -- crates/chronos-domain/src/ports/session.rs` shows no new method on `SessionRepository` |
| **AP-B3** | B3: `store → native` belongs to REC-C3.4, NOT Tren B | Any new edge from `chronos-store::*` to `chronos-native::*` in the Tren B diff | `git diff main..HEAD -- crates/chronos-store` produces no `use chronos_native::` lines |
| **AP-B4** | B4: Composition root split mandatory | Adding session-scoped factories for `advance` / `step` when the existing `&ProbeContext<'_>` chain suffices; relocating `::new()` / `try_open()` calls outside `chronos-mcp::composition` | `git diff main..HEAD -- crates/chronos-mcp/src/composition.rs` keeps the bootstrap-only invariant comment accurate; no new `Default::default()` calls on `SessionStore` outside `composition.rs` |
| **AP-B5** | B5: No `#[ignore]`, no `--skip` | Any `#[ignore]` introduced in a new test; any `--skip` introduced in CI / test invocations | `git diff main..HEAD` produces no `#[ignore]` lines in `crates/chronos-*/tests/` or `chronos-sandbox/tests/`; CI workflow diff shows no `--skip` flag |
| **AP-B6** | B6: REC-C1.5.2 strict replay surfaces stay untouched | Any modification to `crates/chronos-log/src/replay.rs`, `crates/chronos-log/src/segmented.rs`, or `ReplayIntegrityError` variants; any relaxation of `build_replay_plan` to skip integrity checks | `git diff main..HEAD -- crates/chronos-log` returns empty for Tren B; `ReplayIntegrityError` still has 9 variants |
| **AP-B7** | B7: No squash, no rebase, no merge commit | (a) `git merge --squash` on the cycle branch; (b) any force-push / rebase; (c) merge commits in `feat/rec-c3.3-train-b` | `git log --merges main..feat/rec-c3.3-train-b` returns empty; full SHA chain preserved from `fa5eb582` |
| **AP-B8** | B8: Findings are `no_action_in_current_cycle`, not permanent waivers | Any `#[allow]` / `cfg(skip)` / `TODO: waiver` style permanent suppression; findings logged as "waived" instead of `no_action_in_current_cycle` with owner cycle | `git diff main..HEAD` produces no `waived` annotations; `apply-checkpoint.findings_introduced.no_action[]` lists each finding with `owner_cycle` |
| **AP-B9** | B9: Honesty over coverage | Comments that describe behavior the code does not verify (e.g., a comment claiming "replay integrity preserved" without a test asserting it); assertions deleted to make a test pass | Code review per slice; `comments_match_code` lint at the slice boundary; no test deleted in Tren B without a `findings_introduced.no_action` entry citing the specific test by name and path |

## Verification Lattice (summary)

- **T0** (`cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`) must remain green after each slice.
- **T1** (`cargo test --workspace --lib`) must remain green (634 existing tests + new ones).
- **T2** for slices that touch one or two crates — per `AGENTS.md` tier table.
- **T3** (`cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e`) must remain green.
- **T4-smoke** must include `chronos-sandbox/tests/native_probe_tools.rs` once produced (Slice B), and exercises `capture_session` / `probe_advance` / `probe_step` end-to-end against the live `chronos-mcp` binary via the CIH-B `McpTestClient::start` harness.

## Traceability Index (REQ → slice → REQ for archive)

```
REQ-TB-01  → Slice F    REQ-TB-02  → Slice F    REQ-TB-03  → Slice F
REQ-TB-04  → Slice E,G  REQ-TB-05  → Slice E,G  REQ-TB-06  → Slice E,G
REQ-TB-07  → Slice E,G  REQ-TB-08  → Slice C    REQ-TB-09  → Slice C
REQ-TB-10  → Slice C    REQ-TB-11  → Slice D    REQ-TB-12  → Slice D
REQ-TB-13  → Slice D    REQ-TB-14  → Slice A    REQ-TB-15  → Slice A
REQ-TB-16  → Slice B,C,D  REQ-TB-17  → Slice B,C,D,E,F,G
```

## Open Questions Surfaced (do NOT block spec; resolve during design / tasks)

These were marked as unresolved in the exploration report. The spec phase records them so the design phase can resolve them; they do not block the spec being committed.

1. `StoreKind` disclosure (`is_persistent()` on the new port vs separate capability port) — defer to `sddk-design`.
2. `SessionArchive::delete` semantics for live sessions (m1.5 lifecycle-safe delete) — service-layer guard vs port-level `Force` flag — defer to `sddk-design`.
3. `CounterexampleBundleFilter` derive surface (after the move to `chronos-domain`) — defer to `sddk-design`.

## Out-of-Scope (per B3, locked rules)

1. `store → native` direction (REC-C3.4 owns it).
2. Wiring the declared-but-unused `ProbeFactory` / `ProbeRegistry` ports — logged as `no_action_in_current_cycle` per B8.
3. Any modification to `crates/chronos-log/**` — B6.