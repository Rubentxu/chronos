# M7-04 — `session_start` + `session_stop` + `capabilities` (Scoping)

**Cycle:** `feat/m7-04-session-start-stop-capabilities-scoping` (B-direct, scoping)
**Path:** B-direct (documentation cycle — no production code changes)
**Author:** orchestrator
**Date:** 2026-09-11
**Status:** scoping proposal — **OPEN**, awaits m7-04 execution kickoff

> **Naming.** "M7" here refers to the v2-spec sub-cycle that follows the
> M6 sub-cycle (closed 2026-09-11). It is **not** the same as the
> reconstruction-roadmap M7 (*Differential execution v2*) in
> `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` line 163.

---

## Why this cycle

The M7 scoping doc (`docs/milestones/m7-events-read-scoping.md`
§Proposed M7 cycle split) classified m7-04 as:

> `m7-04` (`session_start` + `session_stop` + `capabilities`) —
> Touches session lifecycle; depends on m7-02 (observe) for the
> capabilities surface.

m7-01, m7-02, m7-03 have shipped (tags `m7-01-events-read-merge.0`,
`m7-02-observability-merge.0`, `m7-03-session-compare-explain-merge.0`).
With the diff/observe/compare/explain surface stable, m7-04 is the
next-pending cycle on the M7 backlog. The M7 close report
(`docs/milestones/m6-close-report.md` §6) marks `session_start`,
`session_stop`, and `capabilities` as the three remaining v2 spec
gaps not addressed by any prior cycle.

This scoping cycle **re-validates** the m7-04 shape against the
actual session-lifecycle code and the v2 spec now that
m7-01 + m7-02 + m7-03 have landed. It produces a scoped proposal the
downstream m7-04 execution cycle can execute against, and ships the
**m7-04 cycle spec** for the deliverable.

This cycle does **not** refactor any production code. It is
docs-only.

---

## Current session-lifecycle tool surface (M7-04 entry)

The Chronos MCP server today exposes three session-lifecycle v1
tools, plus several adjacent tools that overlap in functionality:

* `probe_start` — spawn a process under a probe and start collecting
  events. At `crates/chronos-mcp/src/server.rs:3417` (~62 LoC of MCP
  body + security gate + JSON envelope). Returns a `session_id` plus
  bus capacity / language / status. Calls
  `chronos_services::probe::ProbeService::start`.
* `probe_stop` — stop a live probe, drain remaining events, build a
  `QueryEngine`, make the session queryable. At
  `crates/chronos-mcp/src/server.rs:3480` (~52 LoC). Returns
  `session_id`, `status: "stopped"`, `total_events`, `duration_ms`,
  `ebpf_detached`. Calls
  `chronos_services::probe::ProbeService::stop`.
* `probe_drain` — drain current events from a live probe *without*
  stopping it. At `crates/chronos-mcp/src/server.rs:3529` (~60 LoC).
* `session_save` — persist a session's events + metadata to the
  store. (Lives next to `save_session`.)
* `session_load` — load a persisted session back into memory.
* `session_drop` / `drop_session` — remove a session from memory
  (and optionally from the store).
* `session_snapshot` — snapshot a session's state to disk.

None of these map cleanly to the v2 spec's three tools
(`session_start`, `session_stop`, `capabilities`):

* `probe_start` is **only** for live probes; the v2 `session_start`
  must also handle attach-to-existing-pid, attach-to-stored-session,
  and start-from-snapshot paths.
* `probe_stop` returns a small fixed tuple; the v2 `session_stop`
  must also "seal tail" — write a tail marker into the session log
  so downstream consumers know the producer has ended.
* **No `capabilities` tool exists today.** The closest analogue is
  each tool's own `description` string, which is neither
  machine-readable nor session-aware.

---

## v2 spec target

From `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`
lines 11–13:

> - `session_start` — start/attach and return `SessionId` + capability snapshot.
> - `session_stop` — gracefully end producers and seal tail.
> - `capabilities` — available evidence mechanisms for target/session.

Each tool has a distinct semantic intent:

* **`session_start`** is the **session lifecycle entry point**. It
  unifies what is currently three separate paths
  (`probe_start` for new processes, `session_load` for stored
  sessions, and a hypothetical `session_attach` for already-running
  PIDs) behind a single endpoint with an `action` discriminator.
  Returns `session_id` + a **capability snapshot** describing what
  evidence mechanisms the resulting session supports.
* **`session_stop`** is the **graceful-shutdown counterpart**. Beyond
  the current `probe_stop` semantics, it must **seal the tail** —
  emit a tail marker into the session log so consumers can detect
  end-of-stream — and run any on-stop cleanup (final `observe` drain,
  hypothesis-test rollover, etc.).
* **`capabilities`** is a **read-only introspection tool**. It
  enumerates the available evidence mechanisms (event types,
  projections, subscriptions, language adapters) for a target /
  session. Per the spec line 66 ("capability limitations") it is the
  canonical answer to "what can I do with this session?".

The spec is intentionally sparse on the wire shape of all three
tools — see the locked decisions below.

---

## Scope decisions locked in this cycle

The full spec lives at
`docs/milestones/m7-04-session-start-stop-capabilities-merge.md`.
Key scope decisions:

| Decision | Choice | Why |
|---|---|---|
| Unify `probe_start` + `session_load` + a hypothetical attach path under `session_start`? | **Yes** — `session_start` with `action: "spawn" \| "load" \| "attach"` | All three produce a session_id + capability snapshot; unifying avoids three near-identical endpoints |
| `session_start{action=spawn}` preserves v1 `probe_start` parameter set? | **Yes** — `program`, `args`, `cwd`, `trace_syscalls`, `bus_capacity`, `track_function_frames` carry over 1:1 | No agent-visible parameter rename |
| `session_start{action=load}` parameter set? | `session_id` only (load from store). `path` optional override deferred to m7+ | v1 `session_load` is just `session_id` |
| `session_start{action=attach}` parameter set? | `pid: u32` (attach to already-running process). No v1 analogue; net-new for m7-04 | Attach path is in v2 spec line 11 ("start/attach") |
| `session_stop` parameter set? | `session_id`, `seal_tail: bool = true`, `drain_subscriptions: bool = true` | `seal_tail` is the new v2 semantic; `drain_subscriptions` runs the m7-02 observe drain before final stop |
| `capabilities` scope? | `target: Option<TargetSpec>`, `session_id: Option<String>` — at least one required | Both static (target) and dynamic (session) introspection are needed |
| `capabilities` static surface? | `target_event_types: Vec<EventType>`, `target_projections: Vec<ProjectionKind>`, `language: Language`, `language_adapters: Vec<LanguageAdapterStatus>`, `probe_type: ProbeType` | These are the static capabilities a target supports before any session |
| `capabilities` dynamic surface (when `session_id` is given)? | `active_subscriptions: Vec<SubscriptionId>`, `event_types_emitted: Vec<EventType>`, `event_type_counts: HashMap<EventType, u64>`, `bus_capacity: usize`, `bus_fill: usize`, `query_engine_ready: bool` | Dynamic capabilities answer "what is this session currently producing?" |
| Cursor pagination on `capabilities`? | **No** — bounded | Both static and dynamic capability sets are bounded; no cursor needed |
| Cursor pagination on `session_start`/`session_stop`? | **N/A** — both are session-level, not event-level | Sessions are 1:1; no pagination semantics |
| Provenance on all three? | **Yes** (`provenance: SessionLifecycleProvenance`) | v2 spec line 79 + agent ergonomics |
| Preserve v1 `probe_start` as a shim? | **Yes** — routes to `session_start{action=spawn}` | Sunset stays at 2027-09-11 (m6-close-report §4) |
| Preserve v1 `probe_stop` as a shim? | **Yes** — routes to `session_stop` with defaults | Same |
| Preserve v1 `probe_drain` as a shim? | **No** — keep as-is (not a lifecycle endpoint) | `probe_drain` is non-destructive event read; doesn't belong in lifecycle |
| Preserve v1 `session_load` / `session_save` / `drop_session` / `session_snapshot` as shims? | **No** — keep as-is | These are session-persistence tools, distinct from lifecycle |
| New `SessionLifecycleService` module? | **Yes** — `chronos-services/src/session_lifecycle.rs` | Mirrors the dispatcher pattern from m7-01..m7-03 |
| Engine version in provenance? | Hardcoded `"chronos-0.1.0"` (matches m7-02 / m7-03 precedent) | No upstream `engine_version()` API exists |
| `seal_tail` writes a marker where? | Into the session's persisted metadata as `sealed_at: Option<u64>` timestamp; a new `tail_sealed: bool` field on `SessionMetadata` | Avoids introducing a separate "tail log" file format |
| On-stop subscription drain uses which surface? | `ChronosObserveService::observe{verb=list}` (m7-02) | m7-02 dependency already met |

These scope decisions keep m7-04 a focused A-lite path: one
net-new module + three v2 tools + two v1 shims + a single
`SessionMetadata` extension. The `capabilities` tool is the largest
semantic addition; the lifecycle shim conversions are mechanical.

---

## Why A-lite path

m7-01 took A-min, m7-02 took A-full, m7-03 took A-min. m7-04 sits
between them:

1. **2 v1 tools fold into 2 v2 tools** (`probe_start` →
   `session_start{action=spawn}`; `probe_stop` → `session_stop`).
2. **One net-new v2 tool** (`capabilities`) that requires a fresh
   introspection surface (static + dynamic evidence-mechanism
   enumeration).
3. **One net-new `session_start{action=attach}` path** (no v1
   analogue).
4. **Architectural extension**: `SessionMetadata` gains a `tail_sealed`
   boolean and a `sealed_at: Option<u64>` timestamp. This is a
   single-field schema extension, not a structural fork.
5. **Net-new module**: `chronos-services/src/session_lifecycle.rs`
   (~400 LoC estimated) follows the dispatcher precedent from
   m7-01..m7-03.

Estimated LoC delta: **+800 / −80 = net +720**, comparable to m7-02
in volume (715 + 5 shims).

---

## Execution shape (planned)

| Commit | Subject | Files |
|---|---|---|
| 1 | DTOs in `output.rs` (`SessionStartAction`, `SessionLifecycleProvenance`, `SessionStartInput/Output`, `SessionStopInput/Output`, `CapabilitiesInput/Output`, `StaticCapabilities`, `DynamicCapabilities`) | output.rs (+250 LoC) |
| 2 | `SessionMetadata` extension (`tail_sealed`, `sealed_at`) | chronos-store (`SessionMetadata`) (+10 LoC) |
| 3 | Dispatcher + 9 unit tests in `session_lifecycle.rs` | session_lifecycle.rs (new, +400 LoC) |
| 4 | `SessionStartParams` + `session_start` tool wrapper | server.rs (+90 LoC) |
| 5 | `SessionStopParams` + `session_stop` tool wrapper | server.rs (+70 LoC) |
| 6 | `CapabilitiesParams` + `capabilities` tool wrapper | server.rs (+80 LoC) |
| 7 | Shim: `probe_start` → `session_start{action=spawn}` | server.rs (−30 LoC) |
| 8 | Shim: `probe_stop` → `session_stop` | server.rs (−20 LoC) |
| 9 | `lib.rs` module index + T0/T1/T4 gate | lib.rs (+5 LoC) |

Then T4 sandbox smoke (3–4 representative suites — see execution
cycle spec) before FF-merge. `program_scenarios.rs` +
`probe_lifecycle.rs` + `session_lifecycle.rs` are the direct
integration targets; one cross-cutting suite (e.g.
`analytics_tools.rs`) for the new `capabilities` path.

---

## Risks and unknowns

* **Static vs dynamic `capabilities` divergence.** A target's
  static capability set (`target_event_types`, `language`,
  `probe_type`) is mostly deterministic, but `event_types_emitted`
  and `bus_fill` depend on what the producer actually wrote. m7-04
  ships the union of both; agents are expected to ignore fields they
  do not need.
* **`session_start{action=attach}` semantics.** Attaching to an
  already-running PID requires either a separate domain-layer API
  (no current analogue) or a stub that returns
  `ServiceError::Unsupported("attach")` for now. m7-04 picks the
  stub path with the v2 spec line 11 promise documented for m7+ to
  fulfil.
* **`seal_tail` ordering.** Sealing must happen *after* the producer
  has ended (so `probe_stop` runs first) but *before* the
  `QueryEngine` is built (so the engine sees `tail_sealed=true`).
  The dispatcher enforces the order. Documented as a known
  invariant in the module docstring.
* **`drain_subscriptions` on stop.** This calls
  `observe{verb=list}` (m7-02) which is destructive by design.
  Agents that want to preserve fired events must drain before
  `session_stop`. Default-on matches the m7-02 default
  `retention=Drained` semantics; documented.
* **No `capabilities` MCP-suite precedent.** This is the first
  read-only introspection tool in the v2 surface. The shape
  (typed enums + per-field optionality + provenance) is borrowed
  from `observe` (m7-02) and `events_read` (m7-01).
* **No v1 `capabilities` shim.** Because no v1 tool exists with the
  same shape, the v2 tool is net-new with no deprecation migration
  window. This deviates from the M6 standing policy ("preserve v1
  names as deprecated shims") but is justified because there is
  nothing to preserve.
* **`SessionMetadata` additive bump.** Adding `tail_sealed` +
  `sealed_at` is a one-way additive change; both are decorated
  `#[serde(default)]` so old metadata files load with
  `tail_sealed=false, sealed_at=None` (verified during T1 unit
  tests). The `SessionStore::save_session` path needs no change.
* **Sandbox smoke coverage.** `chronos-sandbox/tests/probe_lifecycle.rs`,
  `session_lifecycle.rs`, `program_scenarios.rs` are direct
  integration targets for the shim conversions. m7-04 will rely on
  the existing test rigs; if a `capabilities` smoke test is
  missing it will be added in m7-04 execute.
* **v1 sunset drift.** No new v1 names added; existing
  `probe_start` + `probe_stop` retain the 2027-09-11 sunset.

---

## Exit criteria for this scoping cycle

1. `docs/milestones/m7-04-session-start-stop-capabilities-scoping.md`
   (this file) lands on `main` via FF-merge.
2. `docs/milestones/m7-04-session-start-stop-capabilities-merge.md`
   (the cycle spec for the deliverable) lands on `main` via
   FF-merge.
3. `docs/ROADMAP.md` is unchanged — M7 candidates are already
   listed in the M6 close commit. The scoping cycle is
   documentation-only.
4. `cargo fmt --all -- --check` exits 0.
5. `cargo clippy --workspace --all-targets -- -D warnings` exits 0.

No source code is touched in this scoping cycle. The m7-04 execution
cycle that this scoping seeds is a separate A-lite execution cycle.

---

## Cross-references

* `docs/milestones/m6-close-report.md` §6 — M7 candidate list,
  including the three tools scoped here.
* `docs/milestones/m7-events-read-scoping.md` — M7 split +
  sequencing rationale.
* `docs/milestones/m7-02-observability-merge.md` — closest
  dispatcher precedent (m7-02 A-full + 5 v1 shims; the new
  `SessionLifecycleService` borrows the verb/typed-input pattern
  from `ChronosObserveService`).
* `docs/milestones/m7-03-session-compare-explain-merge.md` —
  closest dispatcher pattern for `session_start{action=spawn}`
  (v1 shim routing).
* `docs/milestones/m7-03-session-compare-explain-scoping.md` —
  scoping template this doc was modelled on.
* `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`
  lines 11–13 (the three tools scoped here) + line 28–42
  (subscription model that `capabilities` reflects) + line 64–66
  (agent ergonomics — provenance, capability limitations, stable
  IDs).

---

— Submitted 2026-09-11. Awaits m7-04 execute cycle kickoff.
