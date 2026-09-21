//! Contract tests for `ObserveVerb` (H1.3, UAT-G0-02 wire contract).
//!
//! These tests prove that three different representations of the same
//! discriminator — Serde Serialize, Serde Deserialize, and the
//! `as_str()` helper — share exactly the same wire format
//! (`"create"` / `"list"` / `"update"` / `"delete"` / `"query"`
//! snake_case) and that the JSON Schema published by JsonSchema uses
//! the same canonical strings.
//!
//! The bug these tests guard against:
//! `ObserveVerb` already uses `#[serde(rename_all = "snake_case")]`, but
//! no automated regression coverage existed for the contract. The
//! `/tmp/g0.2-wire-smoke/` smoke proved the live binary rejects
//! `frobnicate`, but the unit-level guarantee was missing. This file
//! is the regression net for H1.3.
//!
//! See `docs/roadmap/UAT_CATALOG.md` UAT-G0-02 and the G0.2 exploration
//! report in `cycle-artifacts/.../g0.2-observe-uprobe-migration/`.

use chronos_services::output::ObserveVerb;
use schemars::schema_for;
use serde_json::Value;

// =========================================================================
// Serde Serialize / Deserialize — round-trip and string exactness
// =========================================================================

#[test]
fn serialize_create_yields_create_string() {
    let json = serde_json::to_value(ObserveVerb::Create).expect("serialize Create");
    assert_eq!(json, Value::String("create".to_string()));
}

#[test]
fn serialize_list_yields_list_string() {
    let json = serde_json::to_value(ObserveVerb::List).expect("serialize List");
    assert_eq!(json, Value::String("list".to_string()));
}

#[test]
fn serialize_update_yields_update_string() {
    let json = serde_json::to_value(ObserveVerb::Update).expect("serialize Update");
    assert_eq!(json, Value::String("update".to_string()));
}

#[test]
fn serialize_delete_yields_delete_string() {
    let json = serde_json::to_value(ObserveVerb::Delete).expect("serialize Delete");
    assert_eq!(json, Value::String("delete".to_string()));
}

#[test]
fn serialize_query_yields_query_string() {
    let json = serde_json::to_value(ObserveVerb::Query).expect("serialize Query");
    assert_eq!(json, Value::String("query".to_string()));
}

#[test]
fn deserialize_snake_case_strings_yield_correct_variants() {
    for (s, expected) in [
        ("create", ObserveVerb::Create),
        ("list", ObserveVerb::List),
        ("update", ObserveVerb::Update),
        ("delete", ObserveVerb::Delete),
        ("query", ObserveVerb::Query),
    ] {
        let parsed: ObserveVerb =
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
        ObserveVerb::Create,
        ObserveVerb::List,
        ObserveVerb::Update,
        ObserveVerb::Delete,
        ObserveVerb::Query,
    ] {
        let json = serde_json::to_value(variant).expect("serialize");
        let back: ObserveVerb = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, variant, "round-trip preserved {variant:?}");
    }
}

// =========================================================================
// Negative case — unknown discriminator must reject (matches G0.2 wire smoke)
// =========================================================================

#[test]
fn unknown_variant_returns_error() {
    // G0.2 wire smoke observed: `observe(verb="frobnicate")` → server
    // returned -32602 "unknown variant 'frobnicate', expected one of
    // 'create', 'list', 'update', 'delete', 'query'". This unit test
    // pins the same contract at the Serde level.
    let parsed: Result<ObserveVerb, _> =
        serde_json::from_value(Value::String("frobnicate".to_string()));
    assert!(
        parsed.is_err(),
        "unknown verb 'frobnicate' must fail to deserialize"
    );
}

#[test]
fn pascal_case_is_rejected() {
    // PascalCase is never a valid wire format. Every variant must reject
    // its own PascalCase spelling — the canonical wire format is
    // snake_case, matching the JsonSchema and the JSON-RPC client.
    for pascal in ["Create", "List", "Update", "Delete", "Query"] {
        let parsed: Result<ObserveVerb, _> =
            serde_json::from_value(Value::String(pascal.to_string()));
        assert!(
            parsed.is_err(),
            "PascalCase {pascal:?} must be rejected; canonical wire format is snake_case"
        );
    }
}

#[test]
fn unknown_variant_error_lists_all_known_verb_names() {
    // The error message must enumerate the valid snake_case verbs so
    // agents can self-correct without consulting the docs.
    let parsed: Result<ObserveVerb, _> =
        serde_json::from_value(Value::String("frobnicate".to_string()));
    let err = parsed.unwrap_err().to_string();

    for verb in ["create", "list", "update", "delete", "query"] {
        assert!(
            err.contains(verb),
            "error message must list valid verb {verb:?} (got: {err})"
        );
    }
}

// =========================================================================
// JSON Schema — the schema published to clients must use snake_case
// =========================================================================

#[test]
fn json_schema_publishes_snake_case_values() {
    // H1.3 contract: schema, Serde, helper, and the JSON-RPC client must
    // share the same wire strings. We don't depend on the exact path
    // JsonSchema picks — we assert that all five snake_case strings
    // appear in the published schema and the PascalCase spellings do
    // NOT.
    let schema = schema_for!(ObserveVerb);
    let schema_str = serde_json::to_value(&schema)
        .expect("schema to value")
        .to_string();

    for verb in ["create", "list", "update", "delete", "query"] {
        assert!(
            schema_str.contains(&format!("\"{verb}\"")),
            "JsonSchema must publish '{verb}' as a valid value (got: {schema_str})"
        );
    }
    for pascal in [
        "\"Create\"",
        "\"List\"",
        "\"Update\"",
        "\"Delete\"",
        "\"Query\"",
    ] {
        assert!(
            !schema_str.contains(pascal),
            "JsonSchema must NOT publish PascalCase {pascal} (got: {schema_str})"
        );
    }
}

// =========================================================================
// Helper-consistency — `as_str()` and the Serde round-trip share the same
// strings. This is the operator-visible contract: "Serde, JSON Schema
// and the JSON-RPC client share exactly the same contract".
// =========================================================================

#[test]
fn serde_and_as_str_agree_on_wire_strings() {
    for (variant, expected) in [
        (ObserveVerb::Create, "create"),
        (ObserveVerb::List, "list"),
        (ObserveVerb::Update, "update"),
        (ObserveVerb::Delete, "delete"),
        (ObserveVerb::Query, "query"),
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
