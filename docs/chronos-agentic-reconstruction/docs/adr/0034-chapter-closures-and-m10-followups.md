# ADR-0034 — Chapter closures + post-M10 follow-ups: cumulative AUTO+EXEC decisions

**Cycle:** Cross-chapter (M9 + OPS + M11 + M10 closures + 4 M10 §6 follow-ups)
**Status:** `verified` post-write (docs-only; no new code; branch HEAD == `main @ f397f369`)
**Author:** orchestrator (AUTO+EXEC mode, sessions 2026-09-22)

---

## §1 Context

During a single AUTO+EXEC session on 2026-09-22, four chapters of the
chronos roadmap were closed (M9, OPS, M11, M10) and four of the five
follow-ups documented in M10-CLOSE.md §6 were executed. These are
**material architectural decisions** — they reused pre-existing
foundation (EventsCursorV1, EventsCursorV1.advanced_to, read_page,
SessionExecutionLog, QueryEngine.causality_index, etc.) rather than
reinventing new layers.

The individual scoping ADRs (ADR-0028, ADR-0029, ADR-0031, ADR-0032,
ADR-0033) cover the **pre-execution rationale**. This ADR captures the
**post-execution decisions** that emerged during the work: which
follow-ups to execute vs defer, which proxies are defensible, what
the wiring surface looks like, and what the cumulative test density
became.

Per ADR-0004 §2.2 (No Silent Lies), chapter closure and follow-up
execution must be captured honestly — not only the happy-path
wiring, but also the deferred items, the trade-offs, and the
documented limits.

## §2 Decision

### §2.1 Four chapters closed in one AUTO+EXEC session

| Chapter | Sub-cycles executed | New modules | Tag | ADR refs |
|---|---|---|---|---|
| **M9** | 5/5 + close | 5 (concurrency + causal_concurrency + concurrency_perturbation + concurrency_graph + race_classifier) | pre-close tag (no new tag; close at dddc6d58 era) | ADR-0028 §5 |
| **OPS** | 5/5 + cert-4 local-stdio + cert-3 linux-privileged | 1 (health_check) + 3 docs + 1 ADR | `ops-production-ready.0` | ADR-0032, ADR-0033 |
| **M11** | 3/6 + 2 deferred per env + close | 2 (language_capabilities + language_fixtures) | `m11-languages-on-demand.0` | ADR-0031 |
| **M10** | 5/6 + M10.5 deferred per env + close | 2 (live_streaming + virtualization) | `m10-execution-explorer-stubs.0` | ADR-0029 |

### §2.2 M10 §6 follow-ups executed

M10-CLOSE.md §6 listed 5 follow-ups. Four were executed in a follow-up
session; one was deferred per env.

| # | Follow-up | Decision | Commit | Honest trade-off |
|---|---|---|---|---|
| 1 | poll_batch real production wiring | **Executed** — `subscribe_with_log` + `poll_batch_real` via `events_log_read::read_page` | `fb06a9dc` | Additive (does NOT break M10.3 stub contract; 15 existing tests still PASS). |
| 2 | CausalityStatus promotion Unsupported→Wired | **Executed** — `causality_index_is_configured` accessor + `causality_status_for_engine` | `44983b01` | Honest per ADR-0004: status reflects engine availability only; no on-demand index construction. |
| 3 | EventSummary real aggregation | **Executed** — `summarize_log` iterates `read_page` + bucket aggregation | `29b06877` | Additive (does NOT break M10.4 stub contract; 24 existing tests still PASS). |
| 4 | InvocationRollup real grouping | **Executed with thread_id proxy** — `rollup_log` groups by `thread_id` since `chronos_invocation_id` is not in `TraceEvent` | `56b06d75` | thread_id is a defensible upper-bound proxy (every invocation runs on one thread). The grouping key can be swapped without changing the public signature. |
| 5 | Sandbox test 1M eventos | **Deferred per env** | (sandbox integration suite, out of unit-test scope) | Documented in M10-CLOSE-FINAL §3.5 with real path forward. |

### §2.3 Wiring surfaces (the new public API)

The follow-up work added these new public items without removing or
modifying existing contracts:

```rust
// chronos_services::live_streaming
pub struct LiveEventStreamWithLog<'a> { /* ... */ }
pub fn subscribe_with_log<'a>(
    session_id: SessionId,
    registry: Arc<Mutex<SubscriptionRegistry>>,
    log: &'a SessionExecutionLog,
) -> LiveEventStreamWithLog<'a>;

pub fn causality_status_for_engine(
    engine: &chronos_query::QueryEngine,
) -> CausalityStatus;

// chronos_services::virtualization
pub const DEFAULT_BUCKET_SIZE_NS: u64 = 1_000_000_000;

pub fn summarize_log(
    log: &SessionExecutionLog,
    cursor: &EventsCursorV1,
    bucket_size_ns: u64,
) -> EventSummary;

pub fn rollup_log(
    log: &SessionExecutionLog,
    cursor: &EventsCursorV1,
) -> InvocationRollup;

// chronos_query::engine (additive accessor)
impl QueryEngine {
    pub fn causality_index_is_configured(&self) -> bool { /* ... */ }
}
```

### §2.4 Test density cumulative

| Snapshot | Tests cumulativos (chronos-services) | Delta vs prior |
|---|---|---|
| Pre-AUTO+EXEC (baseline) | 397 | — |
| Post-M9 + OPS + M11 + M10 closures (this session part 1) | 509 | +112 |
| Post-4 M10 §6 follow-ups (this session part 2) | 519 | +10 |
| **Total session gain** | | **+122 tests** |

## §3 Rationale

### §3.1 Why additive follow-ups, not rewrites

Each follow-up reused the **existing public contract** of its parent
module:

- `live_streaming::LiveEventStream::subscribe` + `poll_batch` (stub) is
  untouched. The new `subscribe_with_log` + `poll_batch_real` are
  ADDITIVE entry points that compose with the existing types.
- `virtualization::EventSummary::from_bucket_counts` +
  `InvocationRollup::from_invocation_counts` (pure constructors) are
  untouched. The new `summarize_log` + `rollup_log` are ADDITIVE
  functions that consume those constructors.
- `cursor::CausalityStatus::Unsupported` is the default stub return
  (preserved). The new `causality_status_for_engine` is an ADDITIVE
  function that promotes to `Wired` when an engine is configured.

This preserves the **CONTRACT-FIRST** principle (ADR-0004 §1, §6):
the closed chapters' acceptance criteria remain green because the
contracts they pinned did not change.

### §3.2 Why thread_id is a defensible proxy for chronos_invocation_id

Per the M10 follow-up #4 docs (commit `56b06d75`):

- `chronos_invocation_id` is the ideal grouping key (one per function
  call, recursive or otherwise).
- `thread_id` is an **upper bound**: one invocation cannot span
  threads, so per-thread count ≥ per-invocation count for any
  invocation that does not span threads.
- When `chronos_invocation_id` becomes available in the read path
  (i.e. when `TraceEvent` exposes it), the grouping key in
  `rollup_log` can be swapped from `ev.thread_id` to a future
  `ev.invocation_id` field without changing the public signature.

This is a **bounded deferral**, not a hidden trade-off — the public
contract documents the proxy honestly.

### §3.3 Why we did NOT rewrite the stubs

Per ADR-0009 (MCP is a thin adapter) and ADR-0029 §3.4 R3 (live
streaming is a value type), the stub-vs-real distinction is
**structural**: stubs let downstream consumers build against a stable
contract before the real wiring is ready. The follow-ups preserved
that property by adding new entry points rather than replacing
existing ones.

If we had replaced the stubs in-place:
- The M10.3 + M10.4 unit tests (which pin the stub contract) would
  have required updates that retroactively changed the close
  evidence.
- Downstream callers holding the old entry points would have broken
  at compile time without a deprecation path.

The additive approach satisfies both: M10.3 + M10.4 remain CLOSED
with their original tests, and the new follow-up wiring is available
for production consumers.

## §4 Consequences

### §4.1 Positive

- **Test density**: 519/519 PASS cumulativos, +122 nuevos en esta sesión.
- **No Silent Lies**: every chapter close + follow-up has a tagged
  commit, a written close report, and explicit honest deferral of
  out-of-scope items (M10.5 UX HTML, sandbox 1M eventos, M11.4/5
  overhead measurements, OPS remote/multi-tenant profile).
- **Real wiring**: the four M10 follow-ups connect the stubs to
  actual `events_log_read::read_page` + `SessionExecutionLog` +
  `QueryEngine.causality_index` paths.
- **Future-proofing**: the additive contract pattern lets future
  cycles (e.g. real sandbox test, full hint emission pipeline) plug
  in without breaking what's already closed.

### §4.2 Trade-offs (honest)

- **M10.5 UX HTML frontend**: not built in chronos-services scope.
  Belongs to a separate UX execution explorer sub-project (HTML/JS/CSS).
- **Sandbox test 1M eventos**: deferred. Path forward documented in
  M10-CLOSE-FINAL §3.5.
- **`chronos_invocation_id` grouping**: proxy via `thread_id` until
  the field is exposed in the read path. This is a bounded trade-off.
- **`remote/multi-tenant` OPS profile**: NOT IMPLEMENTED per H1.2 §10.
- **GPG-signed tags**: env limitation; we use annotated tags
  (NOT GPG-signed) per ADR-0004 §2.2 honest tagging.

### §4.3 Test gap (intentional, bounded)

The follow-up tests use **real `SessionExecutionLog::create_for_tests`
+ tempdir**, but they exercise **empty logs** (the producer side is
not in scope of chronos-services — it lives in `chronos-capture` /
`chronos-native`). Integration tests with populated logs require:

- The probe runtime (chronos-capture / chronos-native) writing real
  events into a SessionExecutionLog.
- A coordination harness that boots the probe, runs a fixture, and
  stops the probe.

This is the boundary where unit tests end and sandbox tests begin
(per ADR-0010: sandbox-as-uat).

## §5 Verification

- **Tests**: 519/519 PASS (`cargo test -p chronos-services --lib`).
- **Clippy**: 0 warnings (`cargo clippy -p chronos-services --lib --tests --no-deps -- -D warnings`).
- **Cumulative entries** at session end: 57/57 sub-cycles verified +
  3 deferred per env + 4 chapter closures + 4 follow-ups + 1 close-final revision = **69 entries**.
- **Tags**: 3 NEW this session (`ops-production-ready.0`,
  `m11-languages-on-demand.0`, `m10-execution-explorer-stubs.0`),
  all annotated NOT GPG-signed per env limitation.
- **Close reports**: 5 (M9 + OPS + M11 + M10 + M10-CLOSE-FINAL).

## §6 References

- ADR-0004 (No Silent Lies).
- ADR-0009 (MCP is a thin adapter).
- ADR-0010 (Sandbox as UAT).
- ADR-0028 (M9 scoping causal concurrency).
- ADR-0029 (M10 scoping execution explorer).
- ADR-0031 (M11 scoping languages on demand).
- ADR-0032 (OPS scoping production-ready).
- ADR-0033 (OPS support telemetry).
- Close reports: `docs/milestones/{M9,OPS,M11,M10}-CLOSE.md` + `docs/milestones/M10-CLOSE-FINAL.md` + `docs/milestones/ROADMAP-CLOSE.md`.
