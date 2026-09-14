# Tasks: m9-80 chronos-domain owns the four property-policy primitives

> Tasks are reviewable work units that decompose the spec into PR-sized
> chunks. Each task has its own commit and its own acceptance criteria.
>
> **Revision 2 (2026-09-14T07:13Z)** — split into a layered design
> (domain returns `PropertyHypothesisOutcome`, services wraps with
> `From`). See `spec.md` "Design clarification".

## Task ordering

The tasks are sequenced to keep the working tree green at every step
(no half-applied state). T0 must land first because it defines the
new outcome type that T1-T3 depend on; T4-T5 are independent.

```
T0 ──► T1 ──► T4 ──► T5
   \
    └─► T2 ──► T4
       \
        └─► T3 ──► T4
```

## T0 — Define `PropertyHypothesisOutcome` in domain

**Files:**
- `crates/chronos-domain/src/property.rs` (add the new outcome type)
- `crates/chronos-domain/src/lib.rs` (re-export it)

**Acceptance:**
- A new type `PropertyHypothesisOutcome` (or per-kind variants
  `InvariantOutcome`, `ExistenceOutcome`, `CallPathOutcome`) exists in
  `chronos_domain::property`
- The type is re-exported from `chronos_domain::lib`
- `cargo build -p chronos-domain` exits 0
- `cargo test -p chronos-domain --lib` exits 0

**Commit message:**
> m9-80: define PropertyHypothesisOutcome in domain (T0)

## T1 — Move `eval_invariant` into domain (returns InvariantOutcome)

**Files:**
- `crates/chronos-services/src/hypothesis_test.rs` (remove `fn eval_invariant`)
- `crates/chronos-domain/src/property.rs` (add `pub fn eval_invariant` returning `InvariantOutcome`)
- `crates/chronos-services/src/output.rs` (add `impl From<InvariantOutcome> for HypothesisOutput`)

**Acceptance:**
- `grep -n '^fn eval_invariant\b' crates/chronos-services/src/hypothesis_test.rs` returns 0 matches
- `grep -n '^pub fn eval_invariant\b' crates/chronos-domain/src/property.rs` returns 1 match
- `cargo test -p chronos-services --lib hypothesis_test::tests` exits 0
- The wire-shape `HypothesisOutput::Invariant { verdict, ... }` is
  identical before and after (verified by `git diff` on the output
  assertions in unit tests).

**Commit message:**
> m9-80: move eval_invariant into chronos_domain::property (T1)

## T2 — Move `eval_existence` into domain

Same pattern as T1, for `eval_existence`.

**Commit message:**
> m9-80: move eval_existence into chronos_domain::property (T2)

## T3 — Move `eval_call_path` into domain

Same pattern as T1, for `eval_call_path`.

**Commit message:**
> m9-80: move eval_call_path into chronos_domain::property (T3)

## T4 — Move `observe_property_target` and re-export `Property`

**Files:**
- `crates/chronos-services/src/hypothesis_test.rs` (remove `fn observe_property_target`)
- `crates/chronos-domain/src/property.rs` (add `pub fn observe_property_target`)
- `crates/chronos-domain/src/lib.rs` (`pub use property::Property;`)
- `crates/chronos-services/src/hypothesis_test.rs` (update `match` arms in `test` to call domain API qualified with `chronos_domain::property::` and wrap with `From`)

**Acceptance:**
- `grep -n '^pub use.*property::Property\b' crates/chronos-domain/src/lib.rs` returns 1 match
- The four `match` arms in `HypothesisTest::test` reference
  `chronos_domain::property::eval_*` / `chronos_domain::property::observe_property_target`
- All 13 `hypothesis_test` unit tests still pass
- `cargo test -p chronos-services --lib` exits 0 (all 264 tests)

**Commit message:**
> m9-80: move observe_property_target + re-export Property + delegate from services (T4)

## T5 — Final verification + drift check (reviewer-only)

**Files:**
- `crates/chronos-domain/src/property.rs` (final review)
- `crates/chronos-domain/src/lib.rs` (final review)
- `crates/chronos-services/src/hypothesis_test.rs` (final review)
- `crates/chronos-services/src/output.rs` (final review)

**Acceptance:**
- `cargo fmt --all -- --check` exits 0
- `cargo clippy --workspace --all-targets -- -D warnings` exits 0
- `cargo test -p chronos-domain --lib` exits 0
- `cargo test -p chronos-services --lib` exits 0
- The 3 spec scenarios in `spec.md` are mechanically checkable
- No new `clippy::all` allows, no `#[allow(...)]` covers

**Commit message:**
> m9-80: final verification (fmt + clippy + tests) (T5)

## Out-of-scope tasks (deferred to future cycles)

- `evaluate_hypothesis` MCP tool — re-introduce as a real MCP entry
  point using the now-domain-owned functions.
- Property observation trait — generalise `observe_property_target`
  into a `PropertyObserver` trait so non-hypothesis callers can reuse it.

## Cross-checks this cycle must satisfy

- CC#51 — every cycles/index.md row needs a cycle-artifacts folder.
  m9-80 will get one after the cycle closes (post-release update).
- CC#4 — bumping any cycles/index.md row requires regenerating the
  affected archive-manifest index SHAs.
- CC#28 — `release-receipt.md` must have a `Base SHA` field.
- CC#30B — `verify-findings.json` must have `subject.verdict`.
- CC#36 — `verify-findings.json` must have `lens_summary`.
- CC#42 — `release-receipt.md` must use the flat
  `Remote tag | v…` / `Remote tag_peel | <sha>` format.

## Tests pinned

13 unit tests in `crates/chronos-services/src/hypothesis_test.rs` plus
all 264 unit tests in `chronos-services --lib` plus the
`chronos-domain --lib` suite (~30 tests). Plus T5 smoke (sandbox
subset): `e2e_connectivity` + `session_lifecycle` (one hypothesis
attach case).
