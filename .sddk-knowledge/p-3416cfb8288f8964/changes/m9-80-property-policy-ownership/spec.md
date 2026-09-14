# Spec: m9-80 chronos-domain owns the four property-policy primitives

> Spec is the contract. Implementation, layering and file-level diffs live in
> `proposal.md` and `design.md`; tasks live in `tasks.md`.

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

## REQ-M9-80-01 — Domain ownership of the four primitives

`chronos_domain::property` SHALL expose four `pub fn`s — `eval_invariant`,
`eval_existence`, `eval_call_path`, `observe_property_target` — that
take the same arguments and return the same `HypothesisOutput`
envelopes as the current services-layer implementations. The
implementations SHALL be **byte-equivalent** for the existing 13 unit
tests (no behavioural change).

`Property` SHALL be re-exported from the crate root (`chronos_domain`),
so that the docstring in `crates/chronos-mcp/src/server.rs:4949` is
no longer misleading.

**Scenarios:**
- `domain_property_has_four_pub_functions` — `grep -E
  'pub fn (eval_invariant|eval_existence|eval_call_path|observe_property_target)'
  crates/chronos-domain/src/property.rs` returns 4 matches.
- `domain_lib_root_exports_property` — `grep -E 'pub use (self::)?property::Property'
  crates/chronos-domain/src/lib.rs` returns >=1 match.
- `services_layer_delegates_to_domain` — `grep -E
  'chronos_domain::property::(eval_invariant|eval_existence|eval_call_path|observe_property_target)'
  crates/chronos-services/src/hypothesis_test.rs` returns >=4 matches
  (one per function, the call site inside `HypothesisTest::test`).

## REQ-M9-80-02 — Public signature preservation

`chronos_services::hypothesis_test::HypothesisTest::test` SHALL retain
its current signature:

```rust
pub async fn test(
    ctx: &HypothesisTestContext<'_>,
    input: HypothesisInput,
) -> Result<HypothesisOutput, ServiceError>
```

Internally the four `match` arms SHALL delegate to the domain API
rather than calling the now-removed services-layer private `fn`s. The
private `fn eval_invariant`, `fn eval_existence`, `fn eval_call_path`,
and `fn observe_property_target` SHALL NOT exist in
`crates/chronos-services/src/hypothesis_test.rs` after the cycle
completes.

**Scenarios:**
- `services_layer_has_no_private_property_eval_fns` — `grep -E '^fn (eval_invariant|eval_existence|eval_call_path|observe_property_target)\('
  crates/chronos-services/src/hypothesis_test.rs` returns 0 matches.
- `hypothesis_test_public_signature_unchanged` — `git diff
  --unified=0 -- crates/chronos-services/src/hypothesis_test.rs | grep
  '^-    pub async fn test\b'` returns 0 matches (i.e. the line was
  not removed).
- `hypothesis_test_match_arms_call_domain` — the four `match` arms in
  `test` contain `chronos_domain::property::` qualified calls.

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
- **No new property policy types.** The domain re-export is `Property`
  + the four `pub fn`s; existing `PropertySequenceOutcome`,
  `PropertyViolation`, etc. continue to live as leaf exports.
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
