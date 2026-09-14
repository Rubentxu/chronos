# m9-80 T0 Progress Note

> Cycle: `p-3416cfb8288f8964/m9-80-property-policy-ownership`
> Date: 2026-09-14T07:22Z
> Status: T0 complete; T1-T5 pending (next session).

## What landed in T0

Commit `90c7d4c` on branch `feat/m9-80-property-policy-ownership`
(pushed to origin):

**Files changed (2):**
- `crates/chronos-domain/src/property.rs` (+103 LoC, -2 LoC)
- `crates/chronos-domain/src/lib.rs` (+3 LoC, -0 LoC)

**Types defined (7):**

| Type | Role |
|---|---|
| `PropertyHypothesisVerdict` | Pass / Violation / Unsupported discriminator |
| `PropertyObservationSource` | EventCount / PropertyValue / LatencyMs selector |
| `InvariantOutcome` | Invariant-shape policy result |
| `PropertyExistencePredicate` | VariableRead / EventTypeOnly selector |
| `ExistenceOutcome` | Existence-shape policy result |
| `CallPathOutcome` | CallPath-shape policy result |
| `PropertyHypothesisOutcome` | Discriminated union of the three |

All types derive `Debug`, `Clone`, `PartialEq`, `serde::Serialize`,
`serde::Deserialize`. The verdict enum derives `Eq`.

## Verification (from `90c7d4c` commit message)

- `cargo build --workspace` exits 0 (33s wall)
- `cargo test -p chronos-domain --lib`: 149 passed
- `cargo test -p chronos-services --lib hypothesis_test`: 13 passed
- No `use chronos_services` introduced in `chronos-domain`

## Next steps

T1 — Move `eval_invariant` into `chronos_domain::property`:

1. Add `pub fn eval_invariant(
       events: &[TraceEvent],
       comparison: ComparisonOp,
       constant: PropertyValue,
       observation: PropertyObservationSource,
   ) -> InvariantOutcome`
2. Body is a near-byte-for-byte translation of the existing
   `services::hypothesis_test::eval_invariant` but constructs
   `InvariantOutcome` (domain types) instead of
   `HypothesisOutput::Invariant` (services types).
3. Remove `fn eval_invariant` from
   `crates/chronos-services/src/hypothesis_test.rs`.
4. Add `impl From<InvariantOutcome> for HypothesisOutput` in
   `crates/chronos-services/src/output.rs`.
5. Update `HypothesisTest::test` `match` arm for `Invariant` to call
   the domain function and `From::from`.
6. Run `cargo test -p chronos-services --lib` (must stay at 264 passed).

T2 — Same pattern for `eval_existence`.
T3 — Same pattern for `eval_call_path`.
T4 — Move `observe_property_target` + re-export `Property` at the
domain crate root + final delegation wiring.
T5 — fmt + clippy + tests + verify-report.md + implementation-receipt.md.

## Open decisions deferred to T1

1. **Function signature for `eval_invariant`**: should it take
   `PropertyObservationSource` directly, or should it take
   `Option<String>` (target) and infer the source? The latter is closer
   to the existing signature; the former is more typed. **Default**:
   take `PropertyObservationSource` (more typed, clearer contract).
2. **Where do `observe_property_target`'s callers come from?**
   `eval_invariant` calls it when `observation == PropertyValue { ... }`.
   In T1 we add `observe_property_target` to the domain too (or split
   into T1+T4). **Default**: add the helper to the domain in T1
   (private `fn`, called from `eval_invariant`); re-expose as `pub fn`
   in T4 along with the re-export of `Property`.

## Carry-forward

- FIND-M9-75, FIND-M9-74, FIND-M9-72, FIND-M9-71: unchanged.
- FIND-M9-66 (broken JSON in m9-66 verify-findings.json): unchanged.
- Pre-existing smoke-test flake: unchanged.
