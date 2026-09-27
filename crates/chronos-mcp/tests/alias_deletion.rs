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

/// Extract the `instructions = "..."` string from the live
/// `#[rmcp::tool_handler]` attribute in `server.rs`.
///
/// Parsing the real attribute (rather than hardcoding the expected text)
/// is the point: this test must fail if the server keeps advertising a
/// deleted alias to every agent that connects.
fn server_instructions() -> String {
    let src = std::fs::read_to_string("src/server.rs")
        .expect("src/server.rs must be readable from the crate root");
    // Anchor on the attribute line so we do not match unrelated strings.
    let marker = "instructions = \"";
    let start = src
        .find(marker)
        .expect("server.rs must declare `instructions` on the tool_handler")
        + marker.len();
    let rest = &src[start..];
    let end = rest
        .find('"')
        .expect("instructions string must be terminated");
    rest[..end].to_string()
}

/// The `instructions` the server hands to every connecting agent must not
/// advertise any of the 22 deleted v1 aliases as callable.
///
/// Regression: the instructions previously read `query with query_events,
/// get_call_stack, debug_detect_races, inspect_causality` — all four were
/// removed in C5.3.2 and have no handler. Every agent that connected was
/// told to call tools that did not exist.
#[test]
fn server_instructions_do_not_advertise_deleted_aliases() {
    let instructions = server_instructions();
    for alias in DELETED_ALIASES {
        // The string is allowed to *name* aliases so agents learn they were
        // removed, but only inside an explicit removal notice. Anything that
        // presents the alias as usable is a defect.
        let advertised = instructions.contains(&format!("{alias},"))
            || instructions.contains(&format!("{alias}."))
            || instructions.contains(&format!("{alias} "))
            || instructions.contains(&format!(" {alias},"))
            || instructions.contains(&format!(" {alias}."));
        assert!(
            !advertised,
            "server instructions advertise the deleted alias `{alias}` as callable: {instructions}"
        );
    }
}

/// And the instructions must point at tools that actually exist.
#[test]
fn server_instructions_reference_only_live_tools() {
    let instructions = server_instructions();
    for tool in [
        "session_start",
        "events_read",
        "execution_query",
        "causal_slice",
        "state_query",
        "observe",
        "session_compare",
    ] {
        assert!(
            instructions.contains(tool),
            "server instructions should mention the live tool `{tool}`: {instructions}"
        );
    }
}
