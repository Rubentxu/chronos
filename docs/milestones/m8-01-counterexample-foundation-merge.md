# m8-01 — counterexample foundation merge doc

**Cycle:** M8 §1, first execute cycle of the M8 multi-cycle plan.
**Predecessor doc:** `docs/milestones/m8-01-counterexample-foundation-scoping.md` (PROPOSED, FF-merged before this cycle).
**Branch:** `feat/m8-01-counterexample-foundation` (FF-merged to `main` after this cycle).
**Status:** MERGED — 2026-09-11

## What shipped

### 1. New workspace dependency: `proptest = "1.5"`

`Cargo.toml` (workspace root): added `proptest = "1.5"` under `[workspace.dependencies]`, alongside `bincode`, `gimli`, `memmap2`, etc.

`crates/chronos-services/Cargo.toml`: added `proptest = { workspace = true }` under `[dev-dependencies]`.

**Disclosure:** proptest is a `dev-dependency` for m8-01 (not a runtime dep). The actual runtime shrink loop wiring is m8-02's concern. The dep is wired at the workspace level so future m8-02+ cycles can promote it to a runtime dep without a Cargo.toml migration cycle change.

### 2. New service module: `chronos-services::counterexample` (18th service)

`crates/chronos-services/src/counterexample.rs` (342 lines, including tests + helpers).

Exposes:

```rust
pub struct CounterexampleContext<'a> { pub store: &'a chronos_store::SessionStore, pub hypothesis_ctx: &'a HypothesisTestContext<'a> }
pub enum CounterexampleShrinkInput { Shrink { ... }, Get { bundle_id: String }, List { ... } }
pub enum CounterexampleOutput { Got { summary: CounterexampleBundleSummary }, Listed { summaries: Vec<CounterexampleBundleSummary>, next_cursor: Option<String> } }
pub struct CounterexampleBundleSummary { bundle_id: String, property_kind: HypothesisKind, workspace_id: String, created_at_ms: u64, rounds_used: u32, has_full_bundle: bool }
#[allow(dead_code)] pub enum CounterexampleBundle { Owned { summary: ..., events: Vec<TraceEvent> } }
pub struct ChronosCounterexampleService;
pub struct CounterexampleListFilter { pub workspace_id: Option<String>, pub property_kind: Option<HypothesisKind>, pub since_ms: Option<u64>, pub until_ms: Option<u64>, pub limit: u32 }
impl ChronosCounterexampleService {
    pub fn get(&CounterexampleContext<'_>, &str) -> Result<CounterexampleOutput, ServiceError> { Err(LoadFailed("counterexample_bundles table not yet provisioned (m8-03)")) }
    pub fn list(&CounterexampleContext<'_>, CounterexampleListFilter) -> Result<CounterexampleOutput, ServiceError> { Err(Unsupported("counterexample list requires the redb counterexample_bundles table (m8-03)")) }
    // no `shrink` in m8-01 — defer to m8-02
}
pub const DEFAULT_SHRINK_MAX_ROUNDS: u32 = 64;
```

Module registered in `crates/chronos-services/src/lib.rs` at line 84 (between `browser_probe` and `debug_read`).

### 3. New wire DTOs in `chronos-services::output` (3 types)

`crates/chronos-services/src/output.rs` (append, ~110 lines, after the M6 tripwire tests).

```rust
pub struct CounterexampleBundleSummaryDto { bundle_id, property_kind: HypothesisKind, workspace_id, created_at_ms: u64, rounds_used: u32, has_full_bundle: bool }
pub struct CounterexampleListOutputDto { bundles: Vec<CounterexampleBundleSummaryDto>, next_cursor: Option<String> }
pub struct CounterexampleGetOutputDto { bundle: CounterexampleBundleSummaryDto, has_full_bundle: bool }
```

All three derive `JsonSchema` with `rename_all = "snake_case"` to match the project's wire-shape convention.

### 4. Tiny additive change to `HypothesisKind`

Added `serde::Serialize` + `#[serde(rename_all = "snake_case")]` to `output::HypothesisKind` (3-line change). Previously derived `JsonSchema + Copy + Eq + serde::Deserialize` only.

**Disclosure:** the new `Serialize` impl produces `"invariant" | "existence" | "call_path"` strings, matching the JsonSchema's already-`snake_case` wire shape. No inbound JSON breakage — roundtrip tests still pass. The change is required because `CounterexampleBundleSummary` and `CounterexampleBundleSummaryDto` embed a `HypothesisKind` field that is *returned to callers* (line 2500 in output.rs).

### 5. Unit tests (8 new, sandbox-free)

**5 in `counterexample::tests` private mod:**

1. `counterexample_context_is_send_when_inner_refs_are_send` — compile-time tripwire via `fn assert_send<T: Send>(_: T)`.
2. `counterexample_get_signature_accepts_bundle_id_string` — type-equality tripwire for the m8-03 stub signature.
3. `counterexample_list_filter_default_has_zero_limit` — verifies `Default` impl of `CounterexampleListFilter`.
4. `counterexample_input_default_max_rounds_is_64` — pins `DEFAULT_SHRINK_MAX_ROUNDS = 64`.
5. `counterexample_bundle_summary_has_full_bundle_false_is_m8_01_state` — pins m8-01's `has_full_bundle == false` invariant.

**3 in `output::counterexample_dto_tests` private mod:**

6. `counterexample_bundle_summary_dto_roundtrip_snake_case` — JSON roundtrip + key naming.
7. `counterexample_list_output_dto_snake_case_keys` — list envelope wire keys.
8. `counterexample_get_output_dto_dedupes_has_full_bundle` — verifies `has_full_bundle` field matches the bundle's.

## Test results

| Tier | Command | Result |
|---|---|---|
| **T0 — fmt + clippy** | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | PASS, 0 warnings |
| **T0 detail** | clippy exit code | `0` |
| **T1 — lib unit** | `cargo test --workspace --lib --no-fail-fast --exclude chronos-native --exclude chronos-sandbox --exclude chronos-e2e` | **806 passed, 0 failed** (was 798 before m8-01; +8 net) |
| **T2 — per-crate integration** | `cargo test -p chronos-services -p chronos-store -p chronos-mcp --tests --no-fail-fast` | All green: 221 (services) + 76 (services tests) + integration green |
| **T4-smoke — sandbox** | n/a | **not required** — m8-01 doesn't touch probes/sessions. Sandbox binding lives in m8-03. |

No regressions. No ignored tests touched. No `#[ignore]` added.

## Honest disclosures (carried forward to m8-02)

1. **`get` and `list` are stubs.** Both return machine-readable stub error messages. MCP wrappers that bind in m8-03 will hit the stubs on first call and need to be exercised post-m8-03. Acceptance for m8-01 is "the signature compiles and the stub returns the documented error", not "the call returns a real bundle".
2. **No `shrink` entry point.** Intentional split. m8-01 lays foundation; m8-02 wires the `HypothesisInput → proptest::Strategy` adapter and the actual shrink loop.
3. **`HypothesisKind` gained `Serialize`.** Additive wire-shape change. Roundtrip-verified via `counterexample_bundle_summary_dto_roundtrip_snake_case`. No inbound JSON breakage.
4. **m8-01 does NOT satisfy the M8 acceptance criterion** (`MILESTONE_ACCEPTANCE.md` § M8 line 78: "A generated failing input shrinks while preserving the same property violation."). That's m8-05.
5. **`proptest` is a `dev-dependency`** in m8-01. m8-02 may promote it to a runtime dep on `chronos-services` only if the shrink loop needs `proptest::TestRunner` at runtime (likely, per the m6-04 pattern).

## What did NOT ship (deferred, per scoping)

| Item | Cycle |
|---|---|
| `proptest::TestRunner` shrink loop wiring | **m8-02** |
| `HypothesisInput → proptest::Strategy` adapter | **m8-02** |
| `counterexample_shrink` MCP wrapper | **m8-03** |
| `counterexample_get` MCP wrapper | **m8-03** |
| `counterexample_list` MCP wrapper | **m8-03** |
| `counterexample_bundles` redb table | **m8-03** |
| Full-bundle wire DTO `CounterexampleBundleDto` (with `Vec<TraceEvent>`) | **m8-03** |
| `crates/chronos-cli` workspace member | **m8-04** |
| M8 close report | **m8-05** |

## Files touched (in this cycle's commit)

| File | Change | Lines |
|---|---|---|
| `Cargo.toml` | Added `proptest = "1.5"` workspace dep | +1 |
| `crates/chronos-services/Cargo.toml` | Added `proptest` dev-dep | +1 |
| `crates/chronos-services/src/lib.rs` | Registered `pub mod counterexample` | +1 |
| `crates/chronos-services/src/output.rs` | Added `Serialize` to `HypothesisKind`; appended 3 DTOs + 3 DTO tests + comments | +119 |
| `crates/chronos-services/src/counterexample.rs` | New module | +342 |
| `docs/milestones/m8-01-counterexample-foundation-scoping.md` | Already FF-merged in the prior scoping cycle | (pre-existing) |
| `docs/milestones/m8-01-counterexample-foundation-merge.md` | **This document** | (this file) |

Total net additions: **~464 LoC** plus this merge doc.

## Commit + tag + release

- Commit: feature commit with conventional message `feat(services): m8-01 counterexample foundation`.
- Chore commit: apply-checkpoint sync.
- Tag: `m8-01-counterexample-foundation.0`.
- Apply-checkpoint: `sddk/changes/m8-01-counterexample-foundation/apply-checkpoint.json` with `head_sha`, `commits`, and `notes[].smoke_subset = null` (m8-01 has no sandbox smoke, per scoping doc).
- Push: `origin main` only (no PR, no follow-up push).

## Pattern compliance with prior cycles

| Pattern | m7-07 precedent | m8-01 conformance |
|---|---|---|
| Doc first, code second | yes | yes (scoping doc FF-merged before this branch) |
| Stub + machine-readable error to avoid follow-up cycle signature change | yes | yes (`LoadFailed("…not yet provisioned (m8-03)")`) |
| Wire DTOs in `output.rs`, internal enums at module scope | yes | yes |
| `ServiceError::Unsupported(String)` reused for new stubs | yes | yes |
| `chrono-cli` workspace member not added if cycle doesn't need it | n/a | yes (m8-04) |
| T0 + T1 + T2 gates pass on cycle close | yes | yes |
| Honest disclosure in cycle merge doc | yes | yes (this document § "Honest disclosures") |

## Cross-references

* `docs/milestones/m8-counterexample-shrinking-scoping.md` — parent doc, multi-cycle plan.
* `docs/milestones/m8-01-counterexample-foundation-scoping.md` — cycle-specific scoping.
* `docs/milestones/m6-04-hypothesis-test.md` — closest architectural precedent; m8-01 ships only step 1 (signatures + stubs + tests) of m6-04's structure (which was 4 in 1).
* `docs/chronos-agentic-reconstruction/docs/specs/RUNTIME_PROPERTIES_AND_SLICING.md` § "Counterexample shrinking" — spec subsections 54-62.
* `docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` § M8 line 78 — end-to-end acceptance criterion (m8-05).

---

— Submitted 2026-09-11. Cycle closed on `main`.
