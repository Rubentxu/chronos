//! # chronos-native
//!
//! Native language (C/C++/Rust) trace adapter using ptrace.
//!
//! This crate provides ptrace-based process tracing for capturing execution
//! events from compiled native binaries on Linux x86_64.

pub mod address_normalizer;
pub mod capture_runner;
pub mod dwarf;
pub mod native_adapter;
pub mod probe_backend;
pub mod ptrace_tracer;
pub mod symbol_resolver;
pub mod syscall_table;

// Perf counters module (feature-gated)
#[cfg(feature = "perf_counters")]
pub mod perf;

pub use address_normalizer::{AddressNormalizer, SymbolOffset, SymbolOffsetNormalizer};
pub use capture_runner::{AttachMode, CaptureEndReason, CaptureResult, CaptureRunner, CaptureState};
pub use dwarf::{BasicLocationEvaluator, DwarfLocationEvaluator, DwarfReader};
pub use native_adapter::NativeAdapter;
pub use probe_backend::NativeProbeBackend;
pub use ptrace_tracer::{PtraceConfig, PtraceEvent, PtraceTracer};
pub use symbol_resolver::{SymbolInfo, SymbolResolver, SymbolResolverError};
pub use syscall_table::resolve_syscall;
