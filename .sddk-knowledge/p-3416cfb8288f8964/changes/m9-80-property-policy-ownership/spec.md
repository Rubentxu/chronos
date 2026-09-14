# Spec: m9-80 chronos-domain owns the four property-policy primitives

> Spec is the contract. **Revision 2 (2026-09-14T07:13Z)** — see
> "Design clarification" below for the byte-for-byte → layered split.

## Background

`chronos-services::hypothesis_test` owns four functions that implement
property-policy semantics against captured traces: `eval_invariant` (line
110), `eval_existence` (line 278), `eval_call_path` (line 403), and
`observe_property_target` (line 240). `chronos_domain::property::Property`
already implements the same family of operations (`evaluate`,
`evaluate_sequence`, `evaluate_violation`); the four services-layer
functions duplicate that policy in a thin layer that is unaware of the
domain type.

The cycle's intent is to move the policy ownership to
`chronos_domain::property` while preserving the public wire shape and
the 13 unit tests in `hypothesis_test`.

## Design clarification (rev 2, 2026-09-14T07:13Z)

The original spec said "byte-for-byte move", but recon at T1 startup
showed the four functions reference services-layer output types
(`HypothesisOutput`, `HypothesisKind`, `HypothesisScope`,
`HypothesisVerdict`, `ExistencePredicate` — all defined in
`crates/chronos-services/src/output.rs` around lines 1247-1378). A
literal byte-for-byte move would force `chronos-domain` to depend on
`chronos-services`, creating a cycle.

The clean layered design is:

- **`chronos_domain::property`** owns:
  - The four functions, each returning a generic
    `PropertyHypothesisOutcome { verdict_kind, support: Vec<u64>,
    counter: Vec<u64>, summary: String }` tuple (or, more idiomatically,
    three distinct enums per kind).
  - `observe_property_target(events, target) -> Option<(PropertyValue,
    Vec<u64>)>` returning the observation + event-id list.

- **`chronos_services::output`** owns the wire-shape types
  (`HypothesisOutput`, `HypothesisVerdict`, etc.) and provides thin
  `From<PropertyHypothesisOutcome>` conversions.

- **`chronos_services::hypothesis_test`** becomes a coordinator that:
  1. Calls the domain function to get the policy outcome.
  2. Wraps it into the wire-shape `HypothesisOutput` via the
     `From` impl.

This is the right layering. It does require defining
`PropertyHypothesisOutcome` (and the three per-kind variants) in the
domain crate, but it avoids the dependency cycle.

## REQ-M9-80-01 — Domain ownership of the policy primitives

`chronos_domain::property` SHALL expose four `pub fn`s —
`eval_invariant`, `eval_existence`, `eval_call_path`,
`observe_property_target` — that take the same arguments and return a
domain-owned outcome type. The policy semantics SHALL be byte-equivalent
to the current services-layer implementations (no behavioural change).

`Property` SHALL be re-exported from the crate root (`chronos_domain`),
so that the docstring in `crates/chronos-mcp/src/server.rs:4949` is
no longer misleading.

**Scenarios:**
- `domain_property_has_four_pub_functions` — `grep -E
  'pub fn (eval_invariant|eval_existence|eval_call_path|observe_property_target)'
  crates/chronos-domain/src/property.rs` returns 4 matches.
- `domain_lib_root_exports_property` — `grep -E 'pub use (self::)?property::Property'
  crates/chronos-domain/src/lib.rs` returns >=1 match.
- `domain_does_not_depend_on_services` — `grep -E '^use chronos_services'
  crates/chronos-domain/src/*.rs crates/chronos-domain/src/**/*.rs`
  returns 0 matches.

## REQ-M9-80-02 — Wire-shape preservation via wrapper

`chronos_services::hypothesis_test::HypothesisTest::test` SHALL retain
its current signature:

```rust
pub async fn test(
    ctx: &HypothesisTestContext<'_>,
    input: HypothesisInput,
) -> Result<HypothesisOutput, ServiceError>
```

Internally the four `match` arms SHALL:
1. Call the domain function (in `chronos_domain::property`).
2. Convert the domain outcome to `HypothesisOutput` via a `From` impl
   (in `chronos_services::output`).

The private `fn eval_invariant`, `fn eval_existence`, `fn eval_call_path`,
and `fn observe_property_target` SHALL NOT exist in
`crates/chronos-services/src/hypothesis_test.rs` after the cycle
completes.

**Scenarios:**
- `services_layer_has_no_private_property_eval_fns` — `grep -E '^fn (eval_invariant|eval_existence|eval_call_path|observe_property_target)\('
  crates/chronos-services/src/hypothesis_test.rs` returns 0 matches.
- `hypothesis_test_public_signature_unchanged` — `git diff
  --unified=0 -- crates/chronos-services/src/hypothesis_test.rs | grep
  '^-    pub async fn test\b'` returns 0 matches.
- `services_output_has_from_impls` — `grep -E 'impl From<PropertyHypothesisOutcome>'
  crates/chronos-services/src/output.rs` returns >=1 match per kind.

## REQ-M9-80-03 — Test invariants preserved

All 13 unit tests in `crates/chronos-services/src/hypothesis_test.rs`
SHALL pass without modification of their assertions (test bodies MAY
gain `use` imports to reach the domain API). The integration tests
that touch `PropertyValue` (in `crates/chronos-services/tests/`) SHALL
also continue to pass.

**Scenarios:**
- `all_hypothesis_test_unit_tests_pass` — `cargo test -p
  chronos-services --lib hypothesis_test` exits 0 with 13 passed.
- `all_chronos_services_lib_tests_pass` — `cargo test -p
  chronos-services --lib` exits 0 with no failed tests.
- `property_value_integration_tests_pass` — `cargo test -p
  chronos-services --tests --no-fail-fast` exits 0.

## Non-goals

- **No new MCP tool.** The `evaluate_hypothesis` tool advertised in
  `crates/chronos-mcp/src/server.rs:4949` remains a docstring; the
  underlying capability is reachable via
  `session_explain{kind=hypothesis}`. Introducing a real
  `evaluate_hypothesis` MCP tool is deferred to a future cycle.
- **No new property policy types** beyond the `PropertyHypothesisOutcome`
  family needed for the layered split.
- **No generalization of `observe_property_target` into a trait.**
  Defer until a non-hypothesis caller appears.

## Out-of-scope follow-ups (carry forward)

- FIND-M9-75, FIND-M9-74, FIND-M9-72, FIND-M9-71: unchanged.
- FIND-M9-66 (broken JSON in m9-66 verify-findings.json): unchanged.
- Pre-existing smoke-test flake (work-copy isolation leak): unchanged;
  do not use `scripts/smoke_test_ccs.sh` as a CI gate until the leak
  is fixed.

## Wire-shape impact

**None.** The change is internal to the Rust crate graph. No MCP
schema, no JSON envelope, no `session_*` action signature changes.
The only observable change is that
`chronos_domain::property::Property` becomes reachable as
`chronos_domain::Property` (the re-export).

## Revised tasks (replaces the v1 task list)

The five T1-T5 tasks in `tasks.md` need a small revision to reflect
the layered split:

- **T1** — Define `PropertyHypothesisOutcome` (and per-kind variants)
  in `chronos_domain::property`. Add unit tests for the outcome type.
- **T2** — Move `eval_invariant` into `chronos_domain::property`,
  returning `PropertyHypothesisOutcome::Invariant`. Add `From<…> for
  HypothesisOutput` in `services::output`.
- **T3** — Move `eval_existence` and `eval_call_path` (same pattern).
  Add the corresponding `From` impls.
- **T4** — Move `observe_property_target`. Add the
  `From` for the existence path that wraps the observation.
- **T5** — Update `HypothesisTest::test` to call domain + `From`
  instead of services-layer private fns. Run full test suite + clippy +
  fmt.

Tasks.md will be re-edited to match this revision in the next session.
