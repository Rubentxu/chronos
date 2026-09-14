# Tasks — m9-93: TraceEvent JsonSchema derive

## Phase 1: Workspace dep update

### T1.1: Add `jsonschema` feature to workspace uuid

**File**: `Cargo.toml` (root workspace)

**Change**: Update workspace `uuid` dep to include `jsonschema` feature:
```toml
uuid = { version = "1", features = ["v4", "v7", "serde", "jsonschema"] }
```

**Verify**: `cargo metadata --format-version=1 | jq '.packages[] | select(.name=="uuid") | .features'` shows `jsonschema` in the feature list.

**Commit**: `m9-93: enable uuid jsonschema feature (closes InvocationId schema derive)`

## Phase 2: chronos-domain derives

### T2.1: Add `JsonSchema` derive to `Language`

**File**: `crates/chronos-domain/src/trace/session.rs` line 9

**Change**: Add `schemars::JsonSchema` to the derive list on `pub enum Language`.

**Verify**: `cargo build -p chronos-domain` succeeds.

**Commit**: rolled into T2.7 (or separate; TBD).

### T2.2: Add `JsonSchema` derive to `VariableScope` + `VariableInfo`

**File**: `crates/chronos-domain/src/value/typed.rs` lines 8 and 31

**Change**: Add `schemars::JsonSchema` to both derive lists.

### T2.3: Add `JsonSchema` derive to `SourceLocation`

**File**: `crates/chronos-domain/src/trace/location.rs` line 6

**Change**: Add `schemars::JsonSchema` to the derive list.

### T2.4: Add `JsonSchema` derive to `SymbolId` + `InvocationId`

**File**: `crates/chronos-domain/src/trace/event.rs` lines 123 and 152

**Change**: Add `schemars::JsonSchema` to both derive lists.

### T2.5: Add `JsonSchema` derive to `EventType` + `RegisterState` + `WasmModuleInfo` + `WasmFunctionInfo`

**File**: `crates/chronos-domain/src/trace/event.rs` lines 19, 419, 442, 457

**Change**: Add `schemars::JsonSchema` to the four derive lists.

### T2.6: Add `JsonSchema` derive to `EventData` + `TraceEvent`

**File**: `crates/chronos-domain/src/trace/event.rs` lines 231 and 472

**Change**: Add `schemars::JsonSchema` to both derive lists.

### T2.7: Add `JsonSchema` derive test in chronos-domain

**File**: `crates/chronos-domain/src/trace/event.rs` (append to existing `#[cfg(test)] mod tests` block)

**Add**:
```rust
#[test]
fn trace_event_implements_json_schema() {
    use schemars::schema_for;
    let schema = schema_for!(TraceEvent);
    let json = serde_json::to_value(&schema).expect("serialize schema");
    // Top-level must be an object schema with `properties`
    assert!(json.get("properties").is_some(), "schema missing properties");
    let props = json.get("properties").unwrap().as_object().unwrap();
    for required in &["event_id", "timestamp_ns", "thread_id",
                      "event_type", "location", "data"] {
        assert!(props.contains_key(*required),
                "missing property {required} in TraceEvent schema");
    }
}

#[test]
fn event_data_schema_is_oneof() {
    use schemars::schema_for;
    let schema = schema_for!(EventData);
    let json = serde_json::to_value(&schema).expect("serialize schema");
    let one_of = json.get("oneOf").or_else(|| json.get("anyOf"))
        .expect("EventData schema must be a oneOf/anyOf");
    let variants = one_of.as_array().expect("oneOf/anyOf must be array");
    assert!(variants.len() >= 10,
            "EventData should have >=10 variants, got {}", variants.len());
}
```

**Commit**: `m9-93: add JsonSchema derive to TraceEvent + transitive deps (chronos-domain)`

## Phase 3: chronos-services cleanup

### T3.1: Drop `#[schemars(skip)]` from `CounterexampleBundleEventsOutputDto::returned_events`

**File**: `crates/chronos-services/src/output.rs` line 2558

**Change**: Remove the `#[schemars(skip)]` attribute on the `returned_events` field.

### T3.2: Add `CounterexampleBundleEventsOutputDto` schema test

**File**: `crates/chronos-services/src/output.rs` (append to existing `#[cfg(test)] mod tests` block in the same file or a new file)

**Add**:
```rust
#[cfg(test)]
mod bundle_events_schema_tests {
    use super::CounterexampleBundleEventsOutputDto;
    use schemars::schema_for;

    #[test]
    fn dto_schema_includes_returned_events() {
        let schema = schema_for!(CounterexampleBundleEventsOutputDto);
        let json = serde_json::to_value(&schema).expect("serialize");
        let props = json.get("properties")
            .and_then(|p| p.as_object())
            .expect("properties must be object");
        assert!(props.contains_key("returned_events"),
                "CounterexampleBundleEventsOutputDto schema must include \
                 returned_events (m9-93 closed the m9-91 skip)");
        // Confirm returned_events is an array
        let returned = props.get("returned_events").unwrap();
        assert_eq!(returned.get("type").and_then(|t| t.as_str()),
                   Some("array"),
                   "returned_events must be a JSON Schema array");
    }
}
```

**Commit**: `m9-93: drop schemars(skip) on CounterexampleBundleEventsOutputDto.returned_events`

## Phase 4: Verification

### T4.1: T0 — lint gate

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

**Expected**: clean.

### T4.2: T1 — lib unit tests

```bash
cargo test --workspace --lib -- --test-threads=1
```

**Expected**: All existing tests pass + 3 new tests pass
(trace_event_implements_json_schema, event_data_schema_is_oneof,
dto_schema_includes_returned_events).

### T4.3: T4-smoke — sandbox subset

Recommend 1 sandbox smoke test:
```bash
cargo test -p chronos-sandbox --test counterexample_tools -- --test-threads=1
```

This subset exercises the counterexample_bundle_events MCP tool
end-to-end via subprocess. Confirms the published JSON Schema now
includes `returned_events`.

### T4.4: CC sweep

```bash
bash scripts/check_vault_drift.sh
```

**Expected**: clean (no vault drift).

## Phase 5: Cycle artifacts + archive

### T5.1: Write 7 cycle artifacts under `cycle-artifacts/p-3416cfb8288f8964/m9-93-trace-event-json-schema/`

Per AGENTS.md standard cycle artifact pattern:

- `apply-checkpoint.json`
- `implementation-receipt.md`
- `merge-receipt.md`
- `release-receipt.md`
- `release-report.md`
- `verify-findings.json`
- `verify-report.md`

### T5.2: Write archive-manifest

**File**: `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-93-trace-event-json-schema/archive-manifest.md`

Include SHA-256 evidence bindings for all 12 modified files.

### T5.3: Update cycles/index.md

Add m9-93 row + bump Total 92 → 93.

### T5.4: Update terms/index.md

Set Last archive = m9-93-trace-event-json-schema.

### T5.5: Run `python3 scripts/regen_manifest_index_shas.py`

Cascade SHA fixpoint across all archive-manifests.

### T5.6: Pre-create tag v0.7.95 at cycle-artifacts commit

Per CC#42 fixpoint-cascade workaround. Tag stays at pre-cascade-fixpoint commit.

### T5.7: `--no-ff` merge into main + cascade SHA bumps + push

Standard pattern: cycle-artifacts commit → cascade commits (SHA-256 fixpoint, m9-90 re-anchor in m9-92 only) → merge → archive + handoff → push to origin.

### T5.8: Write handoff

**File**: `.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-93-trace-event-json-schema-closure-2026-09-14.md`

Include lessons learned + recommendations for next cycle.

### T5.9: Delete cycle branch

Per CC#53 hygiene.
