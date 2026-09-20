# REC-C4 — SOLID + connascence reduction (proposal)

Branch: `feat/rec-c4-solid-connascence` (off `origin/main` @ 75d04447).
Gate owner of: SOLID-001 (gap), CONN-001 (partial), CONN-002 (partial).

## Intent

Reduce interface-segregation and connascence debt identified by the REC-C4
explore (evidence-grounded):

- SOLID-001: `chronos_capture::TraceAdapter` is a 10-method wide trait whose
  debug-inspection surface (get_threads, get_stack_trace, get_variables,
  get_runtime_info, evaluate_expression) has **zero production consumers**
  (debug reads go through QueryEngine; only tests/mocks call them). Only
  Java/Go implement them; 5 impls pay dead surface.
- CONN-001: `TimestampNs = u64` (bare alias) flows through TraceEvent,
  semantic events, notification port, ebpf types, query filters. ~8 producer
  sites mix clock domains implicitly; chronos-log has ~12 wall-clock-ms sites
  sharing the same u64 shape. Reference patterns already exist
  (evidence.rs monotonic_ns + optional wall-clock; CaptureSession
  started_at/started_at_wallclock).
- CONN-002: SessionId/EventSeq/InvocationId/SymbolId are typed; subscription
  and probe identities remain `String` (MCP 56, services 138, store 11
  sites with `session_id: &str` alone).

## Approach — sliced, each with own gates

### C4.1 — SOLID-001: capability split of `chronos_capture::TraceAdapter`
- Split into small capability traits: `CaptureLifecycle` (start/stop/attach),
  adapter identity stays on the core trait (get_language/name), and a
  separate `DebugInspect` capability trait carrying the 5 dead-surface
  methods with the same UnsupportedOperation defaults.
- `TraceAdapter` becomes a composed supertrait so existing impl blocks and
  the factory keep compiling unchanged (forward-only, no big-bang rewrite).
- Production consumers (factory, chronos-ebpf, mcp server) keep using the
  core; tests/mocks that need debug inspection opt into `DebugInspect`.
- Target: SOLID-001 gap -> partial/verified per contract checker semantics.

### C4.2 — CONN-001: distinct time semantics
- Introduce `MonotonicNs` and `WallClockMs` newtypes in chronos-domain
  (zero-cost, serde-transparent) with conversion constructors; keep
  `TimestampNs` as deprecated alias of `MonotonicNs`.
- Annotate (not mass-rewrite) the highest-risk mixing sites: python
  convert.rs hardcoded 0s, ebpf RawEvent timestamp default. chronos-log
  wall-clock-ms sites get typed at their boundary constructors.
- No on-disk format change: newtypes are repr(transparent) u64.

### C4.3 — CONN-002: typed subscription/probe identities (staged by boundary)
- Stage 1: `SubscriptionId` newtype in chronos-domain; MCP observe verb
  signatures typed; stringly conversion confined to the wire edge.
- Stage 2: services observe/tripwires internals typed.
- Stage 3 (optional, measure first): store boundary (11 sites).
- `session_id: &str` params migrate opportunistically to `&SessionId` only
  where the call site already holds a typed id (avoid 200-site churn in one
  slice).

## Out of scope

- REC-C5 API surface reduction (API-001/002 are C5-owned).
- chronos-log internal wall-clock rework beyond typed boundaries.
- Any on-disk/wire format change (would violate byte-compat invariants).

## Risks

- Trait split may ripple into 10 impl blocks + example; contained by
  supertrait composition (impl blocks unchanged).
- CONN-002 stage 1 touches MCP handler signatures; sandbox smoke
  (observe/tripwire suites) is mandatory gate.
