//! Library facade for chronos-cli integration tests.
//!
//! The CLI is primarily a binary, but the core logic lives in `src/` modules.
//! This `lib.rs` re-exports those modules so integration tests in `tests/`
//! can call into the library without depending on the binary entry point.

pub mod replay;
