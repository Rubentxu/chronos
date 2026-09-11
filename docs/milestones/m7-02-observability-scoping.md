# M7-02 — observe merge (Scoping)

**Cycle:** `p-3416cfb8288f8964/m7-02-observability-scoping` (B-direct, scoping)
**Path:** B-direct (documentation cycle — no production code changes)
**Author:** orchestrator
**Date:** 2026-09-11
**Status:** scoping proposal — **OPEN**, awaits m7-02 execution kickoff

> **Naming.** "M7" here refers to the v2-spec sub-cycle that follows the
> M6 sub-cycle (closed 2026-09-11). It is **not** the same as the
> reconstruction-roadmap M7 (*Differential execution v2*).

---

## Why this cycle

The M7 scoping doc (`docs/milestones/m7-events-read-scoping.md` §Proposed
M7 cycle split) classified `observe` as the **largest and most
architecturally risky** of the M7 candidates:

> `m7-02` (`observe`) is the largest and most architecturally risky
> because it unifies three current v1 surfaces (tripwires,
> properties, probe injection) under a single subscription model. It
> deserves its own scoping pass before execution.

This is that scoping pass. It does **not** refactor any code. It
produces a scoped proposal that the downstream m7-02 execution cycle
can execute against, and ships the **m7-02 cycle spec** for the
deliverable.

---

## Current observe-related tool surface (M7-02 entry)

Five v1 MCP tools span the observe-like surface today:

* `tripwire_create` — register a condition-matching subscription.
  At `crates/chronos-mcp/src/server.rs:2908` (35 LoC of MCP body +
  exhaustive `ServiceError` arms + JSON envelope). Calls
  `TripwiresService::create`.
* `tripwire_list` — enumerate subscriptions + **drain** fired events.
  At `crates/chronos-mcp/src/server.rs:2944` (45 LoC). Calls
  `TripwiresService::list`.
* `tripwire_delete` — unregister a subscription by ID. At
  `crates/chronos-mcp/src/server.rs:2990` (40 LoC). Calls
  `TripwiresService::delete`.
* `tripwire_query` — non-destructive snapshot of subscription state.
  At `crates/chronos-mcp/src/server.rs:3031` (35 LoC). Calls
  `TripwiresService::query`.
* `probe_inject` — attach an eBPF uprobe to a running process.
  At `crates/chronos-mcp/src/server.rs:3473` (80 LoC). Calls
  `ProbeService::inject`.

Together: **~235 LoC of MCP glue** plus the underlying
`chronos_services::tripwires` (678 LoC) + the relevant slice of
`chronos_services::probe` (`ProbeService::inject` only, ~115 LoC).

The service layer is mature. The tripwire evaluation loop runs in the
trace-probe layer and is **not** part of this cycle's refactor. m7-02
only changes the *control plane* (subscription CRUD + fired-event
retrieval).

---

## v2 spec target

From `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`
line 14:

> `observe` — create/change typed observation/subscription/instrumentation request.

Combined with the "Unify subscriptions, tripwires and properties"
section (line 28–42):

```text
Subscription
  scope
  condition
  requested evidence
  action
  retention
  consumer/cursor
```

> Tripwire = condition/action subscription. Property = evaluative subscription/projection.

The v2 `observe` tool therefore:

* exposes a **verb discriminator** (`create | list | update | delete |
  query`) on a single endpoint.
* folds `tripwire_create` + `tripwire_delete` + `tripwire_list` +
  `tripwire_query` + `probe_inject` behind that endpoint.
* preserves the v1 names as deprecated shims (per the M6 standing
  policy in `docs/milestones/m6-close-report.md` §4).
* uses a tagged `condition` union: `{kind: "tripwire" | "uprobe",
  ...}` — tripwire conditions map to the existing `TripwireCondition`;
  uprobe conditions carry `(binary_path, symbol_name, pid)` and route
  through `ProbeService::inject` on match.
* has a `retention` field that defaults to `drained` (matches v1
  `tripwire_list` behaviour; preserves the destructive-vs-non-destructive
  split as `drained` vs `retained_until_session_end`).
* carries the agent-ergonomics fields from spec line 60: cursor
  (optional on `list`), completeness, provenance, retention.

---

## Scope decisions locked in this cycle

The full spec lives at `docs/milestones/m7-02-observability-merge.md`.
Key scope decisions:

| Decision | Choice | Why |
|---|---|---|
| `update` verb in m7-02? | Reject with `Unsupported` | No v1 caller demands it; defer to m7+ |
| `properties` requested_evidence kind? | Reject with `UnsupportedProperties` | QueryEngine lacks property snapshot |
| `action=inject_uprobe` allowed? | Yes | `probe_inject` is a v1 tool; preserve |
| Cursor on `verb=list`? | Yes, optional | Matches m7-01 events_read pattern |
| `retention` default? | `drained` | Backwards compatible with v1 `tripwire_list` |
| Cursor validation? | Best-effort | QueryEngine doesn't expose total_pushed |

These scope decisions keep the v2 surface minimal in m7-02 and push
non-load-bearing features (subscription modification, property
projection) to m7+ where the domain layer has caught up.

---

## Why A-full path

The m7-01 execution was A-min (dispatcher + 2 v1 shims). m7-02 is
**A-full** for these reasons:

1. **5 v1 tools fold into 1 v2 tool** (vs. m7-01's 2→1).
2. **New verb (`update`) on the dispatcher**, even if rejected —
   shape still has to be valid.
3. **New `action` discriminator** (`record | notify | inject_uprobe`)
   with a non-trivial `inject_uprobe` branch that calls into
   `ProbeService::inject`.
4. **eBPF / root-only failure paths** must be tested in sandbox smoke
   (AGENTS.md §2 requires T4-smoke for any cycle that touches
   probe/mcp plumbing).
5. **The dispatcher crosses two existing service modules**
   (`tripwires` + the `inject` slice of `probe`), unlike m7-01 which
   stayed within `debug_trace`.

Estimated LoC delta: **+810 / −280 = net +530**, comparable to m7-01
in volume but broader in semantic surface.

---

## Execution shape (planned)

| Commit | Subject | Files |
|---|---|---|
| 1 | DTOs in `output.rs` | output.rs (+200 LoC) |
| 2 | Dispatcher + 12 tests in `observe.rs` | observe.rs (new, +420 LoC) |
| 3 | `ObserveParams` + `observe` tool wrapper in `server.rs` | server.rs (+180 LoC) |
| 4 | Shim: `tripwire_create` + `probe_inject` → `verb=create` | server.rs (−80 LoC) |
| 5 | Shim: `tripwire_list` → `verb=list` | server.rs (−40 LoC) |
| 6 | Shim: `tripwire_delete` → `verb=delete` | server.rs (−30 LoC) |
| 7 | Shim: `tripwire_query` → `verb=query` | server.rs (−30 LoC) |
| 8 | `lib.rs` module index + T0/T1 gate | lib.rs (+5 LoC) |

Then T4 sandbox smoke (1–2 representative suites — see
`docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`
test plan in the spec) before FF-merge.

---

## Risks and unknowns

* **`update` rejection semantics.** The dispatcher must recognise
  `verb=update` and return a stable error. Pick
  `ObserveError::Unsupported("verb=update deferred to m7+")` for
  callers to detect.
* **Properties projection.** Out of scope in m7-02 by decision. The
  `requested_evidence` field accepts only `event_types` for tripped
  subscriptions; properties arrive in m7+ once `QueryEngine` exposes
  a property table.
* **Uprobe + tripwire fusion.** The `action=inject_uprobe` path
  implies "fire the uprobe every time the tripwire condition matches",
  which is a control-plane concept not exercised today. m7-02 will
  register the subscription but **the inject itself happens once at
  subscription creation** (matching today's `probe_inject` semantics).
  True "fire-on-condition uprobe injection" is a domain-layer concern
  not in scope here.
* **Sandbox eBPF availability.** The T4 smoke must skip gracefully on
  hosts without eBPF / root. The existing `chronos-sandbox/tests/m0_acceptance.rs`
  pattern handles this with `#[cfg(...)]` + `Result<_, _>` short-circuits.
  m7-02 will follow that precedent.
* **v1 sunset drift.** The `observe` dispatcher does not add new v1
  names to deprecate; it only reshapes existing ones. Sunset stays
  at 2027-09-11 (m6-close-report §4).

---

## Exit criteria for this scoping cycle

1. `docs/milestones/m7-02-observability-scoping.md` (this file) lands
   on `main` via FF-merge.
2. `docs/milestones/m7-02-observability-merge.md` (the cycle spec for
   the deliverable) lands on `main` via FF-merge.
3. `docs/ROADMAP.md` is unchanged — M7 candidates are already listed
   in the M6 close commit. The scoping cycle is documentation-only.
4. `cargo fmt --all -- --check` exits 0.
5. `cargo clippy --workspace --all-targets -- -D warnings` exits 0.

No source code is touched in this scoping cycle. The m7-02 execution
cycle that this scoping seeds is a separate A-full execution cycle.

---

## Cross-references

* `docs/milestones/m6-close-report.md` §6 — M7 candidate list.
* `docs/milestones/m7-events-read-scoping.md` — M7 split + sequencing.
* `docs/milestones/m7-01-events-read-merge.md` — closest precedent (m7-01 dispatcher + 2 shims).
* `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`
  line 14 (observe) + lines 28–42 (Subscription model).
* `docs/milestones/m6-04-hypothesis-test.md` — net-new v2 cycle pattern.

---

— Submitted 2026-09-11. Awaits m7-02 execution kickoff.
