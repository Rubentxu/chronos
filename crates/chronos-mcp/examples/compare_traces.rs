//! Example: Compare two captured sessions to find regressions.
//!
//! Run with: cargo run --example compare_traces --manifest-path crates/chronos-mcp/Cargo.toml

fn main() {
    println!("Chronos: Compare traces example");
    println!();
    println!("To compare two debug sessions via MCP:");
    println!("  1. Capture session A: debug_run {{ \"program\": \"./myapp\", \"auto_save\": true }}");
    println!("  2. Make a code change");
    println!("  3. Capture session B: debug_run {{ \"program\": \"./myapp\", \"auto_save\": true }}");
    println!("  4. Compare: compare_sessions {{ \"session_a\": \"<id_a>\", \"session_b\": \"<id_b>\" }}");
    println!();
    println!("The DiffReport shows:");
    println!("  - Events only in A (removed code paths)");
    println!("  - Events only in B (new code paths)");
    println!("  - Similarity percentage");
    println!("  - Timing delta (performance regression detection)");
}