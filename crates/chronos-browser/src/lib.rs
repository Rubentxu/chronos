//! Chronos browser crate for WebAssembly debugging via Chrome DevTools Protocol.
//!
//! This crate provides CDP-based debugging capabilities for WebAssembly modules
//! running in Chrome/Chromium browsers.

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
