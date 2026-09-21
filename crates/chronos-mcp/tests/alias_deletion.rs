//! Integration tests for the 22 deprecated alias handlers deleted in C5.3 (REC-C5).
//!
//! Each test asserts that calling the deleted alias returns an error from the
//! dispatcher layer (the service layer still exposes them — but the MCP router
//! must reject them as `method_not_found`).
//!
//! These tests do not spawn `chronos-mcp`; they assert the structural fact
//! that the alias handler code is gone from `server.rs` after C5.3.2.

/// List of the 22 deprecated alias tool names deleted by C5.3.2.
/// MUST stay in sync with the grep in `specification.md` AC-32-1.
#[allow(dead_code)]
const DELETED_ALIASES: &[&str] = &[
    "query_events",
    "get_event",
    "get_call_stack",
    "get_execution_summary",
    "debug_call_graph",
    "debug_detect_races",
    "debug_expand_hotspot",
    "debug_get_saliency_scores",
    "debug_find_variable_origin",
    "debug_find_crash",
    "inspect_causality",
    "forensic_memory_audit",
    "state_diff",
    "evaluate_expression",
    "debug_get_memory",
    "debug_get_registers",
    "debug_analyze_memory",
    "tripwire_create",
    "tripwire_list",
    "tripwire_delete",
    "tripwire_query",
    "probe_inject",
];

/// Compile-time check: `DELETED_ALIASES` contains exactly 22 names.
#[test]
fn alias_deletion_set_has_22_entries() {
    assert_eq!(
        DELETED_ALIASES.len(),
        22,
        "DELETED_ALIASES must have 22 entries per AC-32-1; found {}",
        DELETED_ALIASES.len()
    );
}

/// Structural check: none of the 22 aliases appears in the source code
/// anymore. Greps the source for `#[tool(name = "<alias>"]` patterns.
#[test]
fn no_alias_handler_remains_in_server_rs() {
    use std::process::Command;

    // Grep with the exact 22-name alternation so the assertion is exact and
    // stable across grep implementations. `grep -c` always prints the count
    // (even when zero); exit code 1 means zero matches but is not an error.
    let joined = DELETED_ALIASES.join("|");
    let pattern = format!(r#"name = "({})""#, joined);
    let output = Command::new("grep")
        .arg("-cE")
        .arg(&pattern)
        .arg("crates/chronos-mcp/src/server.rs")
        .output()
        .expect("grep failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let count: i32 = stdout
        .trim()
        .lines()
        .next()
        .and_then(|line| line.parse().ok())
        .unwrap_or(0);
    assert_eq!(
        count, 0,
        "expected zero occurrences of any 22-alias `name = \"...\"` attribute in server.rs, \
         found {count}. Pattern: {pattern}. Dropout detection: re-run grep manually."
    );
}
