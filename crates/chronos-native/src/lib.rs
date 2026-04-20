//! # chronos-native
//!
//! Native language (C/C++/Rust) trace adapter using ptrace.
//!
//! This crate provides ptrace-based process tracing for capturing execution
//! events from compiled native binaries on Linux x86_64.

pub mod breakpoint;
pub mod capture_runner;
pub mod native_adapter;
pub mod ptrace_tracer;
pub mod symbol_resolver;

pub use breakpoint::BreakpointManager;
pub use capture_runner::{CaptureEndReason, CaptureResult, CaptureRunner, CaptureState};
pub use native_adapter::NativeAdapter;
pub use ptrace_tracer::{PtraceConfig, PtraceEvent, PtraceTracer};
pub use symbol_resolver::{SymbolInfo, SymbolResolver};
