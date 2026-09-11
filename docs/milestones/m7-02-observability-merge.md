# M7-02 — observe merge cycle spec (PROPOSED)

**Cycle:** `p-3416cfb8288f8964/m7-02-observability-merge`
**Path:** A-full (architectural — unifies 4 tripwire + 1 probe_inject tools under one
subscription model with action verbs)
**Status:** proposed — **awaiting m7-02-scoping signoff**

> **Naming.** "M7" here refers to the v2-spec sub-cycle that follows the
> M6 sub-cycle (closed 2026-09-11). It is **not** the same as the
> reconstruction-roadmap M7 (*Differential execution v2*).

---

## Goal

Replace 5 v1 MCP tools (`tripwire_create`, `tripwire_list`,
`tripwire_delete`, `tripwire_query`, `probe_inject`) with one v2
tool: **`observe`** — a single subscription/tripwire/instrumentation
endpoint per the AGENT_API_V2 spec line 14 + 28–42 (the unified
"Subscription" model).

After m7-02 the chronos-mcp tool count drops by **4** (49 → 45).

---

## Why a dispatcher

The 5 v1 tools split naturally into a 4-verb CRUD surface:

| Verb | Replaces v1 tool(s) | Behaviour |
|---|---|---|
| `create` | `tripwire_create` + `probe_inject` | Register a subscription (pure tripwire or uprobe-injecting tripwire) |
| `list` | `tripwire_list` | Enumerate subscriptions + drain fired events |
| `update` | (none — new verb) | Modify a subscription's condition/action/retention |
| `delete` | `tripwire_delete` | Unregister a subscription |

Plus one non-CRUD verb:

| Verb | Replaces v1 tool | Behaviour |
|---|---|---|
| `query` | `tripwire_query` | Non-destructive snapshot of subscription state |

The 5-way split collapses to **5 verbs on 1 endpoint**. Each v1 tool
becomes a thin shim that emits `mode=<verb>` JSON.

---

## Subscription model (from spec line 32–40)

The v2 `observe` tool carries one `Subscription` body with the six
fields the spec mandates:

```text
observe
  subscription_id  (for update/delete/query)        — optional, route key
  verb             (create|list|update|delete|query) — discriminator
  scope            (session_id | global)            — attached scope
  condition        (TripwireCondition | SymbolProbe | FunctionPattern)  — what to watch
  requested_evidence (event_types | properties | symbol_value)          — what to capture
  action           (record | notify | inject_uprobe) — what to do on match
  retention        (drained | retained_until_session_end | permanent)  — how long to keep
  consumer         (cursor: Option<EventCursor>)     — pagination for the fired-events stream
```

The `observe` envelope is a tagged union: `{kind: "tripwire" | "uprobe",
...}`. Tripwires are the existing `TripwireCondition` + `event_types`
filter. Uprobes are a new shape that wraps `probe_inject`'s
`(binary_path, symbol_name, pid)` triplet and triggers the
`ProbeService::inject` path on match.

`list` returns all active subscriptions (with `kind` discrimination) +
a fired-events list (drained unless `retention=drained` overrides).

---

## Backwards compatibility (v1 shims)

The 5 v1 tools are preserved as deprecated shims per the M6 standing
policy (`docs/milestones/m6-close-report.md` §4). Each shim:

- is marked `description = "Deprecated. Use observe with verb=<verb> instead."`
- parses its existing params, builds a 1:1 equivalent `ObserveInput`,
  calls `ChronosObserveService::observe`, and re-serialises the v1
  JSON shape.
- preserves the v1 destructive-vs-non-destructive semantics:
  `tripwire_list` drains fired events; `tripwire_query` does not. The
  `retention` field of the unified subscription model is the v2 knob
  (default `drained` matches v1 `tripwire_list` behaviour).
- handles the existing `probe_inject` failure variants
  (`ProbeStarting`, `EbpfUnavailable`, `AttachFailed`) by translating
  them to a new `ObserveError::ProbeUnavailable(_)` variant +
  `ObserveError::AttachFailed(_)` variant.

v1 sunset: **2027-09-11** (or extension if v1 names are still called).

---

## Files affected

| Path | Change | LoC delta (est) |
|---|---|---|
| `crates/chronos-services/src/output.rs` | + `ObserveInput`, `ObserveOutput`, `ObserveKind`, `ObserveSubscription`, `ObserveAction`, `ObserveRetention`, `SubscriptionDto`, `ProbeInjectSpec` | +200 |
| `crates/chronos-services/src/error.rs` | + `ProbeUnavailable`, `AttachFailed` variants (or use existing `ProbeInjectResult` shapes) | +6 |
| `crates/chronos-services/src/observe.rs` | NEW — `ObserveContext<'a>`, `ObserveInput`, `ChronosObserveService::observe`, ~12 unit tests | +420 |
| `crates/chronos-services/src/lib.rs` | + register `pub mod observe;` + update module index | +5 |
| `crates/chronos-mcp/src/server.rs` | + `ObserveParams` + `observe` tool + shim conversion for the 5 v1 tools (~300 LoC removed) | +180 / −280 |
| `docs/milestones/m7-02-observability-merge.md` | this file (spec) | new |
| `docs/ROADMAP.md` | mark observe as in-progress | minor |

**Total LoC delta: ~+810 / −280 = net +530**. Comparable to
m7-01-events-read-merge (+745 / −267). This is why the scoping
classifies m7-02 as **A-full** rather than A-min: the LoC is similar
but the **semantic surface** (unified subscription model, new
`verb` discriminator, new `update` verb with no v1 equivalent, new
`inject_uprobe` action) is broader than any prior M5/M6 cycle.

---

## Architectural risks

1. **`update` verb has no v1 precedent.** A subscription can be
   modified post-creation. The semantics (atomic? partial? rejection
   rules?) need to be locked in this cycle. **Recommendation:** reject
   `verb=update` in m7-02 (return `ObserveError::Unsupported`) and
   land it in m7+ once a v1 caller signals demand. Keeps the v2
   surface minimal for now.

2. **`inject_uprobe` action requires root + eBPF.** Cross-platform
   uncertainty. The dispatcher's `action=inject_uprobe` branch will
   call `ProbeService::inject` and surface the same failure variants
   (`ProbeStarting`, `EbpfUnavailable`, `AttachFailed`). Sandbox
   integration tests (chronos-sandbox/tests/m0_acceptance.rs) cover
   the eBPF path; m7-02 will not regress them but should add 1–2
   `observe_tools.rs` smoke cases.

3. **`properties` requested-evidence kind is aspirational.** Spec
   line 42 calls out "Property = evaluative subscription/projection"
   but the events_read DTOs (m7-01) already ship with
   `gap_summary=None` because `QueryEngine` does not expose property
   snapshots. m7-02 will land the dispatcher shape but
   `requested_evidence.kind=properties` will return
   `ObserveError::UnsupportedProperties` until the domain layer adds
   property observation in m7+.

4. **Tripwire firing happens during the trace-probe layer, not the
   dispatcher.** The dispatcher is a control-plane (CRUD over
   subscriptions); the data-plane (event-evaluation + firing) stays in
   `TripwireManager::evaluate`. m7-02 does NOT move the evaluation
   loop. It only changes the *subscription registration* and
   *fired-event retrieval* surfaces.

---

## Scope decisions taken in this spec

| Decision | Choice | Rationale |
|---|---|---|
| `update` verb in m7-02? | **No** — reject with `Unsupported` | No v1 caller demands it; reduce risk |
| `properties` requested_evidence? | **No** — reject with `UnsupportedProperties` | QueryEngine lacks property snapshot; m7+ |
| `action=inject_uprobe` allowed? | **Yes** | `probe_inject` is a v1 tool today; preserve |
| Cursor on `verb=list`? | **Yes, optional** | Matches m7-01 events_read pattern |
| `retention` default? | **`drained`** (matches v1 `tripwire_list`) | Backwards compatible |
| Cursor validation? | **Best-effort** (matches m7-01) | QueryEngine doesn't expose total_pushed |

---

## Test plan

- 12 new unit tests in `events_read`-style for the dispatcher
  (create, list drains, list retained, query idempotent, delete, plus
  error-mapping for `InvalidSubscriptionIdFormat`, `SubscriptionNotFound`,
  `Unsupported`, `UnsupportedProperties`, `ProbeUnavailable`, `AttachFailed`)
- 1 sandbox smoke case in `chronos-sandbox/tests/observe_tools.rs`
  (create + list round-trip; verify fired events appear)
- chronos-mcp lib tests: stable (the shim conversion is glue)
- chronos-services lib tests: +12 (156 → ~168 after the cycle)

---

## Execution shape (proposed)

A-full path, 6–8 commits:

1. **DTOs in `output.rs`** — `ObserveInput`, `ObserveOutput`, etc.
2. **Dispatcher in `observe.rs`** — `ChronosObserveService::observe` + 12 tests
3. **`ObserveParams` + `observe` tool wrapper** in `server.rs`
4. **Shim conversion** for `tripwire_create` + `probe_inject` → `verb=create`
5. **Shim conversion** for `tripwire_list` → `verb=list`
6. **Shim conversion** for `tripwire_delete` → `verb=delete`
7. **Shim conversion** for `tripwire_query` → `verb=query`
8. **`lib.rs` module index update** + T0/T1 gate

T4 sandbox smoke (1–2 representative suites) **mandatory** before merge
per AGENTS.md §2 — observe changes the probe lifecycle surface.

---

## Cross-references

- `docs/milestones/m7-events-read-scoping.md` § Proposed M7 cycle split
- `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md` line 14, 28–42
- `docs/milestones/m6-04-hypothesis-test.md` — net-new dispatcher pattern
- `docs/milestones/m7-01-events-read-merge.md` — closest precedent (tagged-union dispatcher + 2 shims)
- `docs/milestones/m6-close-report.md` §4 — v1 deprecation policy

---

— Drafted 2026-09-11. Awaiting m7-02-scoping signoff and execution kickoff.
