# m9-93-trace-event-json-schema — Closure Handoff (2026-09-14)

## Cycle summary

| Field | Value |
|---|---|
| Cycle ID | m9-93-trace-event-json-schema |
| Path | A-min (single-crate refactor + 1-line DTO cleanup) |
| Branch | feat/m9-93-trace-event-json-schema |
| Tag | v0.7.95 |
| Tag peel (immutable) | 60db8b58bdcf9bb55549dad1e43c9d381b45939c (pre-cascade-fixpoint per CC#42 workaround) |
| Merge SHA | 1642ad3de923e202d5652b61b9837d4abe425317 (--no-ff merge into main) |
| Status | CLOSED, archived, cycle branch deleted |
| Tier | T2 (T0 + T1 + T4-smoke) |

## What this cycle did

Closes **FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA** (opened by m9-91).

Adds `schemars::JsonSchema` derive to 12 chronos-domain types
(TraceEvent + 11 transitive dependencies: EventData, EventType,
RegisterState, WasmModuleInfo, WasmFunctionInfo, SymbolId,
InvocationId, SourceLocation, VariableInfo, VariableScope, Language)
and drops the m9-91 `#[schemars(skip)]` workaround on
`CounterexampleBundleEventsOutputDto::returned_events`.

After m9-93, the `counterexample_bundle_events` MCP tool publishes a
JSON Schema where `returned_events` is fully visible as an array of
`TraceEvent` objects. MCP clients can introspect the full event
stream shape (no more "opaque" field).

Also enables schemars's `uuid1` feature so `InvocationId(pub Uuid)`
can derive JsonSchema.

## Drift delta

None — m9-93 is a Rust-only cycle. No vault CC drift introduced or
closed.

## Commit chain

1. **`2e2b7bd`** — m9-93: add JsonSchema derive to TraceEvent + transitive deps (Rust source)
2. **`60db8b5`** — m9-93: cycle artifacts + knowledge files (apply-checkpoint + change-entry + spec + tasks + exploration)
3. **`6203f9a`** — m9-93: align artifacts to v0.7.95 HEAD 60db8b58 (SHA-cascade fixpoint per CC#42)
4. **`1642ad3`** — Merge branch 'feat/m9-93-trace-event-json-schema' into main (--no-ff)

Tag v0.7.95 pre-created at cycle-artifacts commit 60db8b58 per CC#42
fixpoint-cascade workaround (m9-83 handoff). The tag stays at
60db8b58 even after merge + cascade commits to avoid infinite regress.

## Cycle artifacts

All 7 cycle artifacts written under `cycle-artifacts/p-3416cfb8288f8964/m9-93-trace-event-json-schema/`:

- `apply-checkpoint.json`
- `implementation-receipt.md`
- `merge-receipt.md`
- `release-receipt.md`
- `release-report.md`
- `verify-findings.json`
- `verify-report.md`

Archive-manifest with SHA-256 evidence bindings written at
`.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-93-trace-event-json-schema/archive-manifest.md`.

## Lessons learned

1. **Use schemars's `uuid1` feature, not uuid's `jsonschema` feature**
   (which doesn't exist in uuid v1.x). The schemars feature flag
   `uuid1` enables `JsonSchema for Uuid` through a clean upstream
   integration. Trying to enable a non-existent `jsonschema` feature
   on uuid will fail with a `cargo` error listing available features
   — fast feedback but easy to miss if you assume "uuid should have
   it".
2. **JsonSchema derive propagates transitively through `Option<T>`,
   `Vec<T>`, and nested structs/enums**, but does NOT propagate
   through `serde(transparent)` newtype wrappers that don't have
   their own derive. `InvocationId(pub Uuid)` is a `serde(transparent)`
   newtype, so adding `JsonSchema` to `InvocationId` is what actually
   enables `EventData::Function`'s schema to be generated correctly.
   Don't skip the newtype wrappers.
3. **`cargo fmt` catches long-form `match` chains that should be
   single-line `let ... = one_of.as_array().expect(...)`** — the
   test code I wrote initially had a multi-line match that cargo
   fmt squashed to a single line. Cosmetic but consistent style.
4. **The T4-smoke subset for schema changes is small**:
   counterexample_tools exercises the changed DTO directly;
   e2e_connectivity confirms the server still starts. 13 tests
   total run in ~67 seconds (parallel-safe). No need for the full
   35-test sandbox suite for a non-architectural refactor.

## Out-of-scope / Carry-forward

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88): external `sddk` CLI bug; cannot be fixed in chronos scope.
- **Schema docs site auto-generation** (deferred): the published JSON Schemas are now richer but no docs site consumes them. Separate cycle if/when needed.
- **cc-001-god-module** (deferred to m10+): `counterexample.rs` grew ~330 lines in m9-91; still pending follow-up cycles for keys-split / types-split / schema-split.
- **cc-004-implicit-io-toctou** (deferred to m10+): out of scope for m9+.

## Final state

- `bash scripts/check_vault_drift.sh`: PASS (48 python CCs + 7 bash CCs all clean).
- `python3 scripts/regen_manifest_index_shas.py --check`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace --lib -- --test-threads=1`: 742 pass.
- `cargo test -p chronos-sandbox --test counterexample_tools -- --test-threads=1`: 12/12 pass.
- `cargo test -p chronos-sandbox --test e2e_connectivity -- --test-threads=1`: 1/1 pass.
- v0.7.95 → 60db8b58 (cycle-artifacts; CC#42 workaround).
- m9-93 row added to `cycles/index.md`; Total cycles = 93.
- `terms/index.md` Last archive = m9-93-trace-event-json-schema.
- Cycle branch `feat/m9-93-trace-event-json-schema` deleted per CC#53.

## Next steps

m9-94 is the next cycle. Recommended candidates (from m9-91
roadmap + current backlog):

1. **cc-001-god-module** (deferred to m10+): follow-up cycles for
   keys-split / types-split / schema-split. Each is a B-direct or
   A-min Rust cycle.
2. **cc-004-implicit-io-toctou** (deferred to m10+): separate cycle
   for explicit-IO refactor across chronos-services / chronos-store /
   chronos-mcp.
3. **Schema docs site** (new): publish the now-richer JSON Schemas to
   a docs site (e.g., mdbook + schemars-to-md). B-direct vault +
   docs cycle.
4. **External** (sddk CLI): FIND-M9-81 cannot be fixed in chronos
   scope.

Recommend m9-94 = small B-direct or A-min cycle on god-module
follow-up (e.g., cc-001-keys-split is the smallest and lowest-risk).
cc-004-implicit-io-toctou is too big for A-min and should be the
first m10 cycle.
