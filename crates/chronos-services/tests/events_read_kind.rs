//! Contract tests for `EventsReadKind` (G0.1, UAT-G0-01).
//!
//! These tests prove that three different representations of the same
//! discriminator — Serde Serialize, Serde Deserialize, and the JSON Schema
//! published by JsonSchema — share exactly the same wire format
//! (`"query"` / `"by_id"` snake_case) and that the JSON-RPC client
//! expectation matches.
//!
//! The bug these tests guard against (pre-existing on `main`):
//! JsonSchema published `"query"`/`"by_id"` while Serde Deserialize
//! expected `"Query"`/`"ById"`, so a client following the schema was
//! rejected with `-32602 unknown variant query, expected Query or ById`.
//!
//! See `docs/roadmap/UAT_CATALOG.md` UAT-G0-01 and the G0.1 exploration
//! report in `cycle-artifacts/.../g0.1-events-read-kind-contract-align/`.

use chronos_services::output::EventsReadKind;
use schemars::schema_for;
use serde_json::Value;

// =========================================================================
// Serde Serialize / Deserialize — round-trip and string exactness
// =========================================================================

#[test]
fn serialize_query_yields_query_string() {
    let json = serde_json::to_value(EventsReadKind::Query).expect("serialize Query");
    assert_eq!(json, Value::String("query".to_string()));
}

#[test]
fn serialize_by_id_yields_by_id_string() {
    let json = serde_json::to_value(EventsReadKind::ById).expect("serialize ById");
    assert_eq!(json, Value::String("by_id".to_string()));
}

#[test]
fn deserialize_query_string_yields_query_variant() {
    let parsed: EventsReadKind = serde_json::from_value(Value::String("query".to_string()))
        .expect("deserialize 'query'");
    assert_eq!(parsed, EventsReadKind::Query);
}

#[test]
fn deserialize_by_id_string_yields_by_id_variant() {
    let parsed: EventsReadKind = serde_json::from_value(Value::String("by_id".to_string()))
        .expect("deserialize 'by_id'");
    assert_eq!(parsed, EventsReadKind::ById);
}

#[test]
fn roundtrip_preserves_variant() {
    for variant in [EventsReadKind::Query, EventsReadKind::ById] {
        let json = serde_json::to_value(variant).expect("serialize");
        let back: EventsReadKind = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, variant);
    }
}

// =========================================================================
// Negative case — unknown discriminator
// =========================================================================

#[test]
fn unknown_variant_returns_error_without_leaking_internal_names() {
    // UAT-G0-01: discriminador inválido produce error tipado, nunca fallback.
    // El mensaje NO debe filtrar nombres internos del variant Rust (PascalCase).
    let parsed: Result<EventsReadKind, _> =
        serde_json::from_value(Value::String("unknown".to_string()));
    assert!(parsed.is_err(), "unknown variant must fail to deserialize");

    let err = parsed.unwrap_err().to_string();
    assert!(
        !err.contains("Query") && !err.contains("ById"),
        "error message leaks internal Rust variant names: {err}"
    );
    assert!(
        !err.contains("PascalCase"),
        "error message leaks internal naming convention: {err}"
    );
}

#[test]
fn pascal_case_is_rejected_after_contract_alignment() {
    // After the G0.1 fix, Serde no longer accepts PascalCase because the
    // canonical wire format is snake_case (matches JsonSchema + client).
    // This is the explicit breaking change documented in the G0.1
    // exploration report §8.
    let parsed: Result<EventsReadKind, _> =
        serde_json::from_value(Value::String("Query".to_string()));
    assert!(
        parsed.is_err(),
        "PascalCase 'Query' must be rejected after the contract alignment; \
         canonical wire format is snake_case 'query'"
    );
}

// =========================================================================
// JSON Schema — the schema published to clients must use snake_case
// =========================================================================

#[test]
fn json_schema_publishes_snake_case_values() {
    // UAT-G0-01: schema publicado, Serde, herramienta y respuesta coinciden.
    let schema = schema_for!(EventsReadKind);
    let schema_json = serde_json::to_value(&schema).expect("schema to value");

    // Locate the enum values inside the schema. We don't depend on the
    // exact path JsonSchema picks — we only assert that the strings
    // `"query"` and `"by_id"` appear in the published schema and that
    // the PascalCase variants do NOT.
    let schema_str = schema_json.to_string();

    assert!(
        schema_str.contains("\"query\""),
        "JsonSchema must publish 'query' as a valid value (got: {schema_str})"
    );
    assert!(
        schema_str.contains("\"by_id\""),
        "JsonSchema must publish 'by_id' as a valid value (got: {schema_str})"
    );
    assert!(
        !schema_str.contains("\"Query\""),
        "JsonSchema must NOT publish PascalCase 'Query' (got: {schema_str})"
    );
    assert!(
        !schema_str.contains("\"ById\""),
        "JsonSchema must NOT publish PascalCase 'ById' (got: {schema_str})"
    );
}

// =========================================================================
// Helper-consistency — `as_str()` and the Serde round-trip share the same
// strings. This is what the operator asked for: "Serde, JSON Schema and
// the JSON-RPC client share exactly the same contract".
// =========================================================================

#[test]
fn serde_and_as_str_agree_on_wire_strings() {
    use chronos_services::output::EventsReadKind;
    for (variant, expected) in [
        (EventsReadKind::Query, "query"),
        (EventsReadKind::ById, "by_id"),
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
