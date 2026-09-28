# REC-C3-hexagonal-closure · Etapa B'

## Scope

Rewire `CounterexampleService` to consume the `CounterexampleRepository`
port that Tren B introduced in REC-C3.3.3 slice D but never wired into
the service. After this slice, the service depends on
`Arc<dyn CounterexampleRepository>` via `CounterexampleContext` —
matching the `SessionReader` (B.1+B.2+B.3) and `LifecycleStore` (B.4)
patterns.

## Commit

`62067d27` — refactor(services): decouple CounterexampleService from
concrete SessionStore via CounterexampleRepository port (REC-C3.5-B')

## Port surface extension

The original Tren B port had 4 ops (`save_bundle`, `load_bundle_events`,
`list_bundles`, `count_bundle_events`). To replace the service's direct
call to `SessionStore::load_counterexample_bundle` we added a fifth:

```rust
fn load_bundle(
    &self,
    bundle_id: &str,
) -> Result<Option<CounterexampleBundleRecord>, CounterexampleRepositoryError>;
```

The port's `CounterexampleBundleRecord` carries
`minimised: Option<Vec<u8>>` and `target_hypothesis: Option<Vec<u8>>` as
**opaque port bytes**. The store-side `MinimisedPayload` and
`HypothesisInputWire` types stay in `chronos_store` (see R-roadmap
refinement below); the adapter translates store-typed ↔ bytes via
bincode.

## Wiring

- `chronos-domain/src/ports/counterexample.rs`:
  - `load_bundle` added to the trait + `InMemoryCounterexampleRepository`.
  - Doc comment explains the `Vec<u8>` opaque-bytes choice and points
    at REC-C3.5-B'.1 follow-up for the type-move cycle.
- `chronos-store/src/counterexample_repository.rs`:
  - `load_bundle` implemented; two new helpers
    (`encode_minimised_to_port`, `encode_hypothesis_to_port`) mirror
    the existing `decode_minimised` / `decode_hypothesis` flow used
    by `save_bundle`.
  - No changes to the other 4 ops (already wired).
- `chronos-services/src/counterexample.rs`:
  - `CounterexampleContext.store: &SessionStore` →
    `repository: Arc<dyn CounterexampleRepository>`.
  - All 5 call sites rewired: `get`, `events_count`, `events`,
    `list`, `save`.
  - `counterexample_summary_from_wire` (took
    `&Option<MinimisedPayload>`) split into two narrower helpers
    (`port_record_to_services_summary` for `get`,
    `port_summary_to_services_summary` for `list`) that take
    port-shaped inputs only.
  - `cs::bundle_events_count_or_legacy` import dropped; the
    `summary.events_count > 0 ? summary.events_count : events.len()`
    fallback inlined at both `events_count` and `save` call sites.
  - Three new helpers (`encode_minimised_for_port`,
    `encode_hypothesis_for_port`, `port_record_to_services_summary`)
    contain the only store-typed imports.
  - `cargo dep`: `bincode` added (already a workspace dep).
- `chronos-mcp/src/server.rs`:
  - New field `counterexample_repository: Arc<dyn CounterexampleRepository>`
    alongside `reader` / `lifecycle_store` / `archive`. Built via
    the existing composition helper
    `crate::composition::default_counterexample_repository(store_arc)`.
  - The 5 `CounterexampleContext` builders
    (`server.rs:6534–6707`) updated to clone the Arc.

## Verification

- `cargo check --workspace --all-targets --exclude chronos-sandbox
  --exclude chronos-e2e`: clean.
- `cargo test -p chronos-domain --lib`: 166 passed.
- `cargo test -p chronos-store --lib`: 84 passed (adapter's
  `load_bundle` compiles and roundtrips via the existing tests).
- `cargo test -p chronos-services --lib`: 391 passed (all 15
  CounterexampleService tests green after the context rewiring).
- `cargo test -p chronos-mcp --lib`: 109 passed.
- `cargo build --bin chronos-mcp`: clean. Binary SHA256:
  `7a9123753432bef8ba7afd05b2ed2d5fc6528071c3b0d21cc4f7f940e3c293df`
  (mtime 2026-09-20 13:21:58 +0200).
- T4 smoke: `chronos-sandbox e2e_connectivity` (1 test, 14.40 s) +
  `analytics_tools` (4 tests, 67.32 s) +
  `boundary_conditions` (9 tests, 181.52 s) +
  `program_scenarios` (11 tests, 173.45 s) all green.

## Decoupling check

```text
$ grep -nE "ctx\.store\." crates/chronos-services/src/counterexample.rs
(nothing)

$ grep -rn "use chronos_store::SessionStore" \
    crates/chronos-services/src/counterexample.rs
(nothing — the remaining store-typed imports are the wire-mirror types
MinimisedPayload / HypothesisInputWire / ExistencePredicateWire,
and they're used only inside the encode helpers)
```

## R-roadmap refinement (REC-C3.5-B'.1 follow-up, deferred)

The store-shaped `MinimisedPayload` / `HypothesisInputWire` /
`ExistencePredicateWire` types remain in `chronos_store` rather than
moving to `chronos_domain`. Long-term these belong in the domain
(their natural home), and the three encode/decode helpers in
`counterexample.rs` would disappear. Moving them in this cycle would
churn 43 use sites (per
`grep -rn "MinimisedPayload\|HypothesisInputWire\|ExistencePredicateWire"`),
each touching schema-versioning tests in `ce_storage_tests.rs`. The
bincode-bytes shape keeps the boundary clean for now and is a strict
refinement of the previous "service imports store types directly"
state — every store-type touch in `counterexample.rs` is now confined
to four functions with explicit "this is the B' boundary" doc
comments.

**This refinement was registered in the apply-checkpoint under
`notes[].r_roadmap_refinements[]` for the B' slice.**
