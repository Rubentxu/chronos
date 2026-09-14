# Exploration Report — m9-80-property-policy-ownership

> **Cycle**: `p-3416cfb8288f8964/m9-80-property-policy-ownership`
> **Status**: explore-complete; spec/apply/release/archive deferred to the next session
> **Date explored**: 2026-09-14
> **Branch** (created): `feat/m9-80-property-policy-ownership` (untouched; base `dc51b68`)

## Subject

| Base | Head (current) | CWD | Verified at |
|---|---|---|---|
| `dc51b6820fc65c18f328bdc28f64734eeb51d474` | `dc51b6820fc65c18f328bdc28f64734eeb51d474` (no work yet) | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-14T06:48Z |

The cycle is opened but the branch is uncreated — the SDDK `cycle start`
acquired a lease and registered the cycle in the ledger, but no code work
has happened yet.

## Problem statement (from the previous handoff)

`.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-backlog-blocked-2026-09-12.md`
nominates MS-PROPERTY-POLICY as the canonical next roadmap milestone:

> **entry**: ownership of the four observation_log property-policy functions
> **acceptance**: one domain owner/re-export or replacement callsites plus a
> shared chronos-domain fixture

The handoff text mentions "observation_log property-policy functions", but
no symbol named `observation_log` exists in the repo. Recon showed that
the actual four functions in scope are the **hypothesis-evaluation
primitives** that today live in `chronos-services` but conceptually belong
to `chronos-domain::property`:

## Findings (recon)

### F1 — Property evaluation primitives live in services, not domain

`crates/chronos-services/src/hypothesis_test.rs` (1023 lines, 13 unit tests)
hosts three top-level dispatchers and one observation helper that all
depend on `chronos_domain::property::Property*`:

| Function | Line | Purpose |
|---|---|---|
| `eval_invariant` | 110 | Dispatch `HypothesisInput::Invariant` shape: runs `Property::evaluate` against observed `PropertyValue`s; records support / counter events. |
| `eval_existence` | 278 | Dispatch `HypothesisInput::Existence` shape: predicate scan over captured events. |
| `eval_call_path` | 403 | Dispatch `HypothesisInput::CallPath` shape: BFS reachability in the captured call graph. |
| `observe_property_target` | 240 | Helper: walks `TraceEvent`s, finds variable events whose name matches the target, returns last `PropertyValue` + event IDs. |

All four are `fn` (private, non-`pub`), tied to the `HypothesisInput` shape,
and not re-exported from the services crate. They are domain policy
implemented in the wrong layer.

### F2 — `chronos_domain::property` already has the policy

`crates/chronos-domain/src/property.rs` (1220 lines) owns:

- `Property::evaluate` (line 258) — single-event boolean eval
- `Property::evaluate_sequence` (351) — sequence-window eval, returns `PropertySequenceOutcome`
- `Property::evaluate_violation` (472) — produces `PropertyViolation` bundle
- `Property::violation_transition` (447) — bundle-to-state-transition bridge
- `Property::to_dsl` / `from_dsl` (485/500) — round-trippable policy text

The four `services`-layer functions do not call `evaluate_sequence` or
`evaluate_violation` directly — they implement their own (mostly simpler)
versions of those operations inline.

### F4 — MCP surface claims a tool that does not exist

`crates/chronos-mcp/src/server.rs:4949` docstring describes an
`evaluate_hypothesis` tool:

> Three supported shapes: 'invariant' (reuses
> `chronos_domain::property::Property::evaluate` on EventCount or
> PropertyValue), 'existence' (predicate scan over captured events; Pass if
> >=1 match, else Violation), 'call_path' (BFS reachability in the call
> graph).

The actual MCP tool is `evaluate_expression` (state query, arithmetic
expressions); there is no top-level `evaluate_hypothesis` MCP entry point.
The `hypothesis_test` shape is currently reachable only via the v2
`session_explain{kind=hypothesis}` dispatcher, not as a standalone tool.

This is not a regression to fix in this cycle; it is documented because
moving the four primitives into `chronos-domain` is what makes
re-introducing `evaluate_hypothesis` as a real MCP tool a 1-day follow-up
rather than a re-implementation.

### F5 — HypothesistTest public API is bounded

`pub async fn test` at `hypothesis_test.rs:88` is the only public entry
point; it owns the `HypothesisInput → HypothesisOutput` contract and the
13 tests in the module. Moving the four helpers into `chronos-domain`
will not change `test`'s signature or the wire shape, only its internals.

### F6 — Public surface of `chronos_domain::property`

`crates/chronos-domain/src/lib.rs:28` re-exports leaf types
(`PropertySequenceOutcome`, `PropertyValue`, `PropertyViolation`, …) but
NOT `Property` itself. The MCP docstring's "reuses
`chronos_domain::property::Property::evaluate`" is therefore technically
wrong — callers reach the type as `chronos_domain::property::Property`,
not via the `chronos_domain` root.

## Scope decision

**Path**: A-min (1-3 crates, no architectural fork).

The change touches:
- `crates/chronos-services/src/hypothesis_test.rs` (move 4 functions out)
- `crates/chronos-domain/src/property.rs` (add 4 corresponding functions or
  move them; re-export the type at the crate root)
- `crates/chronos-services/src/output.rs` (re-export the new domain API)
- `crates/chronos-services/src/hypothesis_test.rs` tests (port the 13
  unit tests; rewire to the new call sites)

Total: 2 crates, ~600 LoC movement, no public wire shape change.

## Tiers (planned)

- T0: `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`
- T1: `cargo test -p chronos-domain --lib` (1220-line module, ~30 tests) and
  `cargo test -p chronos-services --lib` (must remain green)
- T2: `cargo test -p chronos-services --tests` (the 13 unit tests + the
  counterexample/replay integration tests that touch `PropertyValue`)
- T4-smoke subset (per AGENTS.md §2 A-min table): one sandbox suite that
  exercises the hypothesis path end-to-end — likely `session_explain`
  (`hypothesis` kind) and `program_scenarios` (the session-stop
  hypothesis-driven shape).
- CC#4 regen + vault drift smoke at the end.

## Out of scope (deferred)

- Re-introducing the missing `evaluate_hypothesis` MCP tool — API-shape
  decision, separate cycle.
- Generalising `observe_property_target` to a proper property-observation
  trait that services-layer code can compose against — defer until the
  first non-hypothesis caller appears.

## Deviations from the handoff

The handoff said "four observation_log property-policy functions" but
`observation_log` does not exist in the repo. The four functions above
are the only plausible match (the only place that has "observation +
property + policy"-shaped code). Confirmed via grep:
`grep -rln 'observation_log' crates/` returns zero results.

## Next phase

This exploration is sufficient to begin **spec** work. Recommended
structure of `spec.md`:

1. State the problem in two paragraphs: services-layer eval functions +
   domain-layer property policy that have drifted apart since the M3
   shrink-loop work.
2. Three requirements:
   - **REQ-PROP-OWN-001** — the four functions live in
     `chronos-domain::property` (one file, one module); `Property` is
     re-exported from the crate root.
   - **REQ-PROP-OWN-002** — `chronos-services::hypothesis_test::test`
     retains its public signature; its four helpers are now thin shims
     that delegate to the domain API.
   - **REQ-PROP-OWN-003** — all 13 unit tests in `hypothesis_test.rs`
     continue to pass without modification of their assertions.
3. Scenarios for each requirement (falsification tests, not new feature
   tests).

## Open follow-ups

None new. The low-severity items from m9-79 (FIND-M9-75, FIND-M9-74,
FIND-M9-72, FIND-M9-71) remain unchanged.

## Handoff for the next session

The exploration report above is the only artifact this cycle has produced.
The branch was created via SDDK (`feat/m9-80-property-policy-ownership`,
base `dc51b68`) but is empty. The lease was acquired with token 1;
**renew the lease before resuming** (`sddk cycle lock renew`) and
continue from `phase.spec.start`.