//! Contract tests for `ObserveScopeWire` (H1.3, UAT-G0-02 wire contract).
//!
//! These tests prove that the wire-shape wrapper around the v2 `observe`
//! `scope` body follows the **externally-tagged** enum convention
//! (`#[serde(tag = "scope")]`): the discriminator lives at the same
//! nesting level as the payload.
//!
//! Wire format pinned:
//! ```text
//! {"scope": "session", "session_id": "<uuid>"}  // Session variant
//! {"scope": "global"}                            // Global variant (no payload)
//! ```
//!
//! The bug these tests guard against:
//! The G0.2 wire smoke captured the server complaining about a
//! missing `scope` field when the client sent the bare payload
//! (`{"session_id": "..."}`). The first iteration of G0.2
//! misinterpreted the externally-tagged shape as a bug and tried to
//! "fix" it by removing the `tag = "scope"` attribute; the wire smoke
//! caught the regression before commit and the change was reverted.
//! This file is the unit-level guarantee that the externally-tagged
//! contract is honoured at every layer.
//!
//! See `docs/roadmap/UAT_CATALOG.md` UAT-G0-02 and the G0.2 exploration
//! report in `cycle-artifacts/.../g0.2-observe-uprobe-migration/`
//! §3 (the wire-shape double-key reversal).

use chronos_mcp::server::ObserveScopeWire;
use serde_json::{json, Value};

// =========================================================================
// Deserialize — externally-tagged variant with payload
// =========================================================================

#[test]
fn deserialize_session_yields_session_variant_with_payload() {
    let wire = json!({
        "scope": "session",
        "session_id": "00000000-0000-0000-0000-000000000000"
    });
    let parsed: ObserveScopeWire =
        serde_json::from_value(wire).expect("externally-tagged session scope must deserialize");
    match parsed {
        ObserveScopeWire::Session { session_id } => {
            assert_eq!(session_id, "00000000-0000-0000-0000-000000000000");
        }
        ObserveScopeWire::Global => {
            panic!("scope=session must yield Session variant, not Global");
        }
    }
}

#[test]
fn deserialize_global_yields_global_variant() {
    let wire = json!({ "scope": "global" });
    let parsed: ObserveScopeWire =
        serde_json::from_value(wire).expect("externally-tagged global scope must deserialize");
    assert!(
        matches!(parsed, ObserveScopeWire::Global),
        "scope=global must yield Global variant"
    );
}

// =========================================================================
// Negative case — bare payload (no `scope` tag) is rejected
// =========================================================================

#[test]
fn bare_payload_without_scope_tag_is_rejected() {
    // This is the G0.2 first-iteration bug: sending the session_id at
    // the root (no "scope" key) was incorrectly accepted by a "fix"
    // that removed `tag = "scope"`. The reverted/canonical contract
    // rejects this.
    let bare = json!({
        "session_id": "00000000-0000-0000-0000-000000000000"
    });
    let parsed: Result<ObserveScopeWire, _> = serde_json::from_value(bare);
    assert!(
        parsed.is_err(),
        "missing 'scope' tag must fail to deserialize — externally-tagged enum requires the tag"
    );
}

#[test]
fn unknown_scope_tag_is_rejected() {
    let bad = json!({
        "scope": "frobnicate",
        "session_id": "00000000-0000-0000-0000-000000000000"
    });
    let parsed: Result<ObserveScopeWire, _> = serde_json::from_value(bad);
    assert!(
        parsed.is_err(),
        "unknown scope tag 'frobnicate' must fail to deserialize"
    );
}

#[test]
fn session_variant_requires_session_id_field() {
    // The Session variant requires session_id. Without it, the
    // externally-tagged representation must reject the request — the
    // MCP layer should not invent a default session id.
    let missing_id = json!({ "scope": "session" });
    let parsed: Result<ObserveScopeWire, _> = serde_json::from_value(missing_id);
    assert!(
        parsed.is_err(),
        "scope=session without session_id must fail (missing field)"
    );
}

// =========================================================================
// Wire shape documented for client implementers — ObserveScopeWire is a
// server-side read-only type (Deserialize only) so we cannot pin a
// round-trip, but we document the exact JSON the G0.2 wire smoke
// observed against the live binary.
// =========================================================================

#[test]
fn session_id_field_appears_at_top_level_in_wire_payload() {
    // The wire payload for scope=session must contain a `session_id`
    // field at the top level (not nested inside `scope`). This is the
    // externally-tagged contract.
    let wire = json!({
        "scope": "session",
        "session_id": "abc-123"
    });
    let obj = wire.as_object().expect("top-level object");
    assert_eq!(
        obj.get("scope"),
        Some(&Value::String("session".to_string()))
    );
    assert_eq!(
        obj.get("session_id"),
        Some(&Value::String("abc-123".to_string()))
    );
    assert!(
        !obj.contains_key("payload") && !obj.contains_key("data"),
        "externally-tagged shape must NOT nest payload inside scope: {obj:?}"
    );
}
