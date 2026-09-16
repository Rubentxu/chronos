//! chronos-sandbox: MCP testing infrastructure for Chronos.
//!
//! Provides a test client for the Chronos MCP server that can:
//! - Spawn MCP server instances as child processes
//! - Send JSON-RPC calls and receive responses
//! - Manage debug sessions and capture probes
//!
//! ## Fixture discovery
//!
//! Tests that need to launch a C harness binary (e.g. `test_busyloop`,
//! `test_segfault`) obtain its path through [`FixtureResolver`]. The
//! resolver reads the absolute fixture root from the compile-time env var
//! `CHRONOS_FIXTURE_DIR` published by [`build.rs`]. This is the only
//! supported way to find a fixture in the harness — production code never
//! touches it.
//!
//! [`build.rs`]: ../build.rs

pub mod client;
pub mod fixture_resolver;
pub mod programs;

pub use client::error::McpSandboxError;
pub use client::{McpSession, McpTestClient};
pub use fixture_resolver::FixtureResolver;
