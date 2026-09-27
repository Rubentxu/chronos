//! `chronos test run` — start a live probe against a target program (STUB).
//!
//! m8-04 ships this subcommand as a stub returning `NotImplemented` with a
//! clear operator-facing message. M9 (causal concurrency) is now CLOSED, but
//! full live-probe plumbing (process spawn, ptrace attach, kernel-event
//! streaming into a fresh `QueryEngine`, then a `hypothesis_test::test` over the
//! live events) is still not landed — see the chronos-roadmap SDDK vault for
//! the cycle plan, and the error message below for the three dependencies
//! that gate it.
//!
//! Why a stub and not a silent "command not found"? An operator running
//! `chronos test run ./hello` against a binary should see a sane message
//! naming the milestone that lands the feature, not an unfriendly fallback.
//! The DB path is still parsed and validated even in stub form, so the
//! operator can see we respected `--db`.

use std::path::Path;

use anyhow::{anyhow, Result};

/// STUB for `chronos test run`. Returns `Err` with a milestone-pinned message.
///
/// We deliberately don't print to stdout/stderr here — the caller in `main.rs`
/// owns the exit code / message display contract. Returning an error lets
/// the binary exit non-zero (operator scriptability: `if chronos test run …`)
/// and lets `main.rs` decorate the message consistently across subcommands.
pub fn run_live_probe_stub(db_path: &Path, args: &[String]) -> Result<()> {
    let _ = db_path; // parsed + validated upstream; intentionally unused here.
    let program = args.first().map(String::as_str).unwrap_or("<program>");
    Err(anyhow!(
        concat!(
            "`chronos test run ",
            "{program}` requires live-probe plumbing not yet wired.\n",
            "M9 (causal concurrency) is CLOSED (see docs/milestones/m9-close.md) but\n",
            "the runtime dependencies are not landed:\n",
            "  1. probe-runtime binding (USDT/BCC/Linux tracepoints, ADR-0023)\n",
            "  2. ChronosServer cohesive sub-context extraction (H1.4-B deferred)\n",
            "  3. remote/multi-tenant OPS profile (NOT IMPLEMENTED)\n",
            "Use `chronos test replay <bundle_id>` against an already-persisted bundle."
        ),
        program = program
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_returns_error_naming_runtime_dependencies() {
        let err = run_live_probe_stub(std::path::Path::new("/tmp/chrono.db"), &["hello".into()])
            .unwrap_err();
        let msg = format!("{err:#}");
        // Post-v0.8.0 honesty: the stub no longer says "m9+ scope" (M9 is
        // CLOSED); it now lists the live-probe runtime dependencies that
        // are still not landed. This pins the post-M9 close contract.
        assert!(msg.contains("requires live-probe plumbing"), "msg: {msg}");
        assert!(
            msg.contains("hello"),
            "msg should mention the program: {msg}"
        );
        assert!(msg.contains("probe-runtime binding"), "msg: {msg}");
        assert!(msg.contains("ChronosServer"), "msg: {msg}");
        assert!(msg.contains("remote/multi-tenant"), "msg: {msg}");
    }

    /// The operator-facing error must render as a readable multi-line list.
    ///
    /// Regression pin: the message body used `\\` (an *escaped backslash*)
    /// where a line continuation was intended, so the binary printed a
    /// literal backslash between every clause and ran them together on a
    /// single unreadable line. This test failed against that version.
    #[test]
    fn stub_error_renders_multiline_without_literal_backslashes() {
        let err = run_live_probe_stub(std::path::Path::new("/tmp/chrono.db"), &["hello".into()])
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            !msg.contains('\\'),
            "message must not leak literal backslashes: {msg}"
        );
        // Each of the three numbered dependencies sits on its own line.
        for n in 1..=3 {
            let needle = format!("\n  {n}. ");
            assert!(msg.contains(&needle), "missing line for item {n}: {msg}");
        }
        assert!(
            msg.ends_with("already-persisted bundle."),
            "hint should terminate the message: {msg}"
        );
    }

    #[test]
    fn stub_handles_missing_program_arg() {
        let err = run_live_probe_stub(std::path::Path::new("/tmp/chrono.db"), &[]).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("<program>"), "msg: {msg}");
    }
}
