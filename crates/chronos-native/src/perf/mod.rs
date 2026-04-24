//! Performance counter support via `perf_event_open`.
//!
//! This module provides hardware performance counter access using Linux's
//! `perf_event_open` syscall. Counters include CPU cycles, instructions,
//! cache misses, and branch misses.
//!
//! # Feature Gate
//!
//! This module is only available when the `perf_counters` feature is enabled.
//!
//! ```toml
//! [dependencies]
//! chronos-native = { path = "...", features = ["perf_counters"] }
//! ```

pub mod counters;

pub use counters::{
    PerfCounterConfig, PerfCounterError, PerfCounterHandle, PerfCounterType,
    PerfCountersSnapshot,
};
