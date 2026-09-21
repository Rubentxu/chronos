//! Contract tests for `SessionStartAction` (H1.3, session_start discriminator).
//!
//! These tests prove that three different representations of the same
//! discriminator — Serde Serialize, Serde Deserialize, and the
//! `as_str()` helper — share exactly the same wire format
//! (`"spawn"` / `"load"` / `"attach"` snake_case) and that the JSON
//! Schema published by JsonSchema uses the same canonical strings.
//!
//! The bug these tests guard against:
//! `SessionStartAction` already uses `#[serde(rename_all = "snake_case")]`,
//! but no automated regression coverage existed for the contract. The
//! v2 server (`crates/chronos-mcp/src/server.rs`) accepts `action=spawn`
//! / `action=load` / `action=attach`; this test pins the wire contract
//! at the Serde level so that any rename to PascalCase, kebab-case, or
//! camelCase will be caught before the dispatcher ever sees the
//! request.
//!
//! See `docs/roadmap/UAT_CATALOG.md` and the v2 surface spec
//! (`rec-c7-convergence-close` for the canonical action taxonomy).

use chronos_services::output::SessionStartAction;
use schemars::schema_for;
use serde_json::Value;

// =========================================================================
// Serde Serialize / Deserialize — round-trip and string exactness
// =========================================================================

#[test]
fn serialize_spawn_yields_spawn_string() {
    let json = serde_json::to_value(SessionStartAction::Spawn).expect("serialize Spawn");
    assert_eq!(json, Value::String("spawn".to_string()));
}

#[test]
fn serialize_load_yields_load_string() {
    let json = serde_json::to_value(SessionStartAction::Load).expect("serialize Load");
    assert_eq!(json, Value::String("load".to_string()));
}

#[test]
fn serialize_attach_yields_attach_string() {
    let json = serde_json::to_value(SessionStartAction::Attach).expect("serialize Attach");
    assert_eq!(json, Value::String("attach".to_string()));
}

#[test]
fn deserialize_snake_case_strings_yield_correct_variants() {
    for (s, expected) in [
        ("spawn", SessionStartAction::Spawn),
        ("load", SessionStartAction::Load),
        ("attach", SessionStartAction::Attach),
    ] {
        let parsed: SessionStartAction =
            serde_json::from_value(Value::String(s.to_string())).expect("deserialize snake_case");
        assert_eq!(
            parsed, expected,
            "deserialize {s:?} should yield {expected:?}"
        );
    }
}

#[test]
fn roundtrip_preserves_all_variants() {
    for variant in [
        SessionStartAction::Spawn,
        SessionStartAction::Load,
        SessionStartAction::Attach,
    ] {
        let json = serde_json::to_value(variant).expect("serialize");
        let back: SessionStartAction = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, variant, "round-trip preserved {variant:?}");
    }
}

// =========================================================================
// Negative case — unknown discriminator must reject
// =========================================================================

#[test]
fn unknown_action_returns_error() {
    // The dispatcher relies on this rejection to surface a clear error
    // to the JSON-RPC client instead of silently defaulting to one of
    // the actions.
    let parsed: Result<SessionStartAction, _> =
        serde_json::from_value(Value::String("frobnicate".to_string()));
    assert!(
        parsed.is_err(),
        "unknown action 'frobnicate' must fail to deserialize"
    );
}

#[test]
fn pascal_case_is_rejected() {
    for pascal in ["Spawn", "Load", "Attach"] {
        let parsed: Result<SessionStartAction, _> =
            serde_json::from_value(Value::String(pascal.to_string()));
        assert!(
            parsed.is_err(),
            "PascalCase {pascal:?} must be rejected; canonical wire format is snake_case"
        );
    }
}

// =========================================================================
// JSON Schema — the schema published to clients must use snake_case
// =========================================================================

#[test]
fn json_schema_publishes_snake_case_values() {
    let schema = schema_for!(SessionStartAction);
    let schema_str = serde_json::to_value(&schema)
        .expect("schema to value")
        .to_string();

    for action in ["spawn", "load", "attach"] {
        assert!(
            schema_str.contains(&format!("\"{action}\"")),
            "JsonSchema must publish '{action}' as a valid value (got: {schema_str})"
        );
    }
    for pascal in ["\"Spawn\"", "\"Load\"", "\"Attach\""] {
        assert!(
            !schema_str.contains(pascal),
            "JsonSchema must NOT publish PascalCase {pascal} (got: {schema_str})"
        );
    }
}

// =========================================================================
// Helper-consistency — `as_str()` and the Serde round-trip share the same
// strings.
// =========================================================================

#[test]
fn serde_and_as_str_agree_on_wire_strings() {
    for (variant, expected) in [
        (SessionStartAction::Spawn, "spawn"),
        (SessionStartAction::Load, "load"),
        (SessionStartAction::Attach, "attach"),
    ] {
        let via_serde = serde_json::to_value(variant)
            .expect("serialize")
            .as_str()
            .expect("string variant")
            .to_string();
        let via_helper = variant.as_str().to_string();
        assert_eq!(
            via_serde, expected,
            "Serde wire string for {variant:?} should be {expected:?}"
        );
        assert_eq!(
            via_helper, expected,
            "as_str() helper for {variant:?} should be {expected:?}"
        );
        assert_eq!(
            via_serde, via_helper,
            "Serde and as_str() must agree for {variant:?}"
        );
    }
}
