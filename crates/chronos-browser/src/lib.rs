//! Chronos browser crate for WebAssembly debugging via Chrome DevTools Protocol.
//!
//! This crate provides CDP-based debugging capabilities for WebAssembly modules
//! running in Chrome/Chromium browsers. It implements the "eBPF probes for WASM"
//! pattern: breakpoints are set via CDP's Debugger domain, with zero overhead
//! when not debugging (`Debugger.disable()`).
//!
//! # Architecture
//!
//! ## Event Flow
//!
//! ```text
//! Chrome (WASM)
//!   → CDP WebSocket
//!   → BrowserCdpClient
//!   → paused_to_wasm_events()
//!   → WasmSemanticResolver
//!   → SemanticEvent
//!   → MCP tools (browser_probe_drain)
//! ```
//!
//! ## Module Overview
//!
//! - [`adapter`](adapter/index.html) — `TraceAdapter` + `ProbeBackend` implementation for browser debugging
//! - [`browser`](browser/index.html) — Chrome process lifecycle (spawn, find, kill)
//! - [`cdp_client`](cdp_client/index.html) — WebSocket CDP communication with typed commands/responses
//! - [`error`](error/index.html) — Typed error variants for browser operations
//! - [`event_mapper`](event_mapper/index.html) — CDP paused events → TraceEvent conversion
//! - [`wasm_detector`](wasm_detector/index.html) — Standalone WASM module detection (can be used independently)
//! - [`wasm_probes`](wasm_probes/index.html) — Breakpoint manager for WASM functions
//! - [`wasm_resolver`](wasm_resolver/index.html) — TraceEvent → SemanticEvent enrichment
//!
//! # Production Readiness
//!
//! - ✅ Async-correct: no nested runtimes, proper tokio usage
//! - ✅ Resource cleanup: Chrome killed on Drop via `BrowserAdapter::drop()`
//! - ✅ Session isolation: unique temp dir per session (`ChromeProcess`)
//! - ✅ Error handling: typed errors with `is_chrome_not_found()` and `is_cdp_error()` helpers
//! - ✅ Zero overhead: `Debugger.disable()` when not probing
//! - ✅ Docker compatibility: `--disable-dev-shm-usage` flag in headless mode
//! - ⚠️ E2E tests require Chrome (`CHRONOS_E2E=1`)
//!
//! # Quick Start
//!
//! ```ignore
//! use chronos_browser::BrowserAdapter;
//! use chronos_domain::CaptureConfig;
//! use chronos_capture::TraceAdapter;
//!
//! // Create adapter and start probing
//! let adapter = BrowserAdapter::new();
//! let config = CaptureConfig::new("http://example.com/wasm.html");
//!
//! // Start capture (spawns Chrome, connects CDP)
//! let session = adapter.start_capture(config)?;
//!
//! // After some time, drain events
//! let events = adapter.drain_events()?;
//!
//! // Stop when done
//! adapter.stop_probe(&session)?;
//! ```
//!
//! # Feature Flags
//!
//! This crate is a library and does not expose feature flags. All functionality
//! is available through the public API.

pub mod adapter;
pub mod browser;
pub mod cdp_client;
pub mod error;
pub mod event_mapper;
pub mod wasm_detector;
pub mod wasm_probes;
pub mod wasm_resolver;

pub use adapter::BrowserAdapter;
pub use browser::ChromeProcess;
pub use cdp_client::BrowserCdpClient;
pub use error::BrowserError;
pub use event_mapper::paused_to_wasm_events;
pub use wasm_detector::WasmModuleDetector;
pub use wasm_probes::WasmBreakpointManager;
pub use wasm_resolver::WasmSemanticResolver;
