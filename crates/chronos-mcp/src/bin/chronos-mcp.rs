//! Chronos MCP Server — Time-Travel Debugging for AI Agents.
//!
//! This binary starts the Chronos MCP server on stdio, ready to be
//! used by any MCP-compatible AI client (Claude, Cursor, etc.).
//!
//! Usage:
//!   chronos-mcp                    # Start server on stdio
//!   RUST_LOG=debug chronos-mcp     # Start with debug logging

use chronos_mcp::ChronosServer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Initialize logging with env filter (RUST_LOG=info by default)
    // Write logs to stderr so stdout is clean for JSON-RPC communication
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Starting Chronos MCP Server v{}", env!("CARGO_PKG_VERSION"));

    // m9-75: fail closed on a store that cannot be opened. Exiting before the
    // transport starts is the only way a client can tell the difference between
    // "your store is there" and "the server quietly started with an empty one".
    let server = match ChronosServer::try_new() {
        Ok(server) => server,
        Err(e) => {
            tracing::error!("Fatal: {}", e);
            eprintln!("chronos-mcp: fatal: {}", e);
            std::process::exit(2);
        }
    };

    if let Err(e) = server.run_stdio().await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
