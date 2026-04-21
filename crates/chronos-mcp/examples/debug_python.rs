//! Example: Capture a Python script execution and query results.
//!
//! Run with: cargo run --example debug_python --manifest-path crates/chronos-mcp/Cargo.toml

fn main() {
    println!("Chronos: Python debug example");
    println!("This example demonstrates the chronos-mcp MCP tool interface.");
    println!();
    println!("To debug a Python script via MCP:");
    println!("  1. Start chronos-mcp: cargo run -p chronos-mcp --release -- --stdio");
    println!("  2. Use an MCP client to call: debug_run");
    println!("     params: {{ \"program\": \"/usr/bin/python3\", \"args\": [\"my_script.py\"] }}");
    println!("  3. Query results with: query_events, reconstruct_call_stack, etc.");
    println!();
    println!("Available MCP tools:");
    let tools = vec![
        "debug_run", "query_events", "get_event", "reconstruct_call_stack",
        "detect_races", "query_causality", "find_variable_origin",
        "get_execution_summary", "expand_hotspot", "get_saliency_scores",
        "save_session", "load_session", "list_sessions", "compare_sessions",
    ];
    for tool in tools {
        println!("  - {}", tool);
    }
}