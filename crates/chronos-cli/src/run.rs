//! `chronos test run` — start a live probe against a target program (STUB).
//!
//! m8-04 ships this subcommand as a stub returning `NotImplemented` with a
//! clear operator-facing message. Full live-probe plumbing (process spawn,
//! ptrace attach, kernel-event streaming into a fresh `QueryEngine`, then a
//! `hypothesis_test::test` over the live events) is m9+ scope — see the
//! chronos-roadmap SDDK vault for the cycle plan.
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
        "`chronos test run {program}` is not yet implemented (m9+ scope). \
         Use `chronos test replay <bundle_id>` against an already-persisted bundle. \
         See docs/propuestas/chronos-roadmap for the milestone plan."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_returns_error_naming_milestone() {
        let err = run_live_probe_stub(std::path::Path::new("/tmp/chrono.db"), &["hello".into()])
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("not yet implemented"), "msg: {msg}");
        assert!(
            msg.contains("hello"),
            "msg should mention the program: {msg}"
        );
        assert!(msg.contains("m9+"), "msg should name the milestone: {msg}");
    }

    #[test]
    fn stub_handles_missing_program_arg() {
        let err = run_live_probe_stub(std::path::Path::new("/tmp/chrono.db"), &[]).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("<program>"), "msg: {msg}");
    }
}
