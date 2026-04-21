//! # chronos-native
//!
//! Native language (C/C++/Rust) trace adapter using ptrace.
//!
//! This crate provides ptrace-based process tracing for capturing execution
//! events from compiled native binaries on Linux x86_64.

pub mod breakpoint;
pub mod capture_runner;
pub mod dwarf;
pub mod native_adapter;
pub mod ptrace_tracer;
pub mod symbol_resolver;
pub mod syscall_table;

pub use breakpoint::BreakpointManager;
pub use capture_runner::{AttachMode, CaptureEndReason, CaptureResult, CaptureRunner, CaptureState};
pub use dwarf::DwarfReader;
pub use native_adapter::NativeAdapter;
pub use ptrace_tracer::{PtraceConfig, PtraceEvent, PtraceTracer};
pub use symbol_resolver::{SymbolInfo, SymbolResolver};
pub use syscall_table::resolve_syscall;
