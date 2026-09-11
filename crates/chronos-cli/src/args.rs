//! Manual argument parser for the `chronos` CLI.
//!
//! B-decision B1 (m8-04 scoping doc): no clap — keeps the binary small and the
//! dependency footprint light. The CLI only exposes two subcommands in m8-04
//! (`test run` and `test replay`), so a hand-rolled `match`-based parser is
//! cheaper than pulling in a third-party framework.
//!
//! # Grammar
//!
//! ```text
//! chronos [--db <path>] test replay <bundle_id>
//! chronos [--db <path>] test run       <program-args...>     # stub in m8-04
//! chronos --help
//! ```
//!
//! The `--db` flag selects the redb path used by `SessionStore::open`.
//! Per B-decision B5, the default is XDG-data: `$XDG_DATA_HOME/chronos/chronos.db`
//! (falling back to `$HOME/.local/share/chronos/chronos.db` when the XDG env
//! is unset — typical Linux convention).
//!
//! # Errors
//!
//! `ArgsError` covers the three failure modes we expect from the operator:
//! unknown subcommand, missing positional, or `--db` flag without a path. We
//! deliberately keep the error surface narrow — the CLI is for human operators,
//! and any other failure mode is a programming error.

use std::path::PathBuf;

/// Top-level CLI command after parsing.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Replay a persisted counterexample bundle.
    ///
    /// Reads the bundle by id from the session store, rebuilds a `QueryEngine`
    /// from the bundle's `events` field, and re-runs the minimised hypothesis
    /// against that synthetic engine. Prints the resulting verdict and the
    /// summary JSON for operator inspection.
    Replay { bundle_id: String, db: PathBuf },
    /// Run the live probe against a target program (STUB in m8-04).
    ///
    /// Full live-probe plumbing is m9+; this subcommand currently returns
    /// `NotImplemented` with a clear message so operators get a sane failure
    /// instead of a cryptic "command not found".
    Run { db: PathBuf, args: Vec<String> },
    /// Print usage.
    Help,
}

/// Failure modes that can arise from argument parsing.
#[derive(Debug, thiserror::Error)]
pub enum ArgsError {
    /// Reserved for future per-subcommand rejection. Today the parser
    /// collapses unknown subcommands into [`ArgsError::Usage`], but
    /// keeping this variant means callers can pattern-match exhaustively
    /// without breaking when we add a stricter dispatch layer.
    #[allow(dead_code)]
    #[error("unknown subcommand: {0}")]
    UnknownSubcommand(String),
    #[error("missing required argument for: {0}")]
    MissingArgument(String),
    #[error("--db requires a path argument")]
    MissingDbPath,
    #[error("usage: chronos [--db <path>] test <run|replay> ...")]
    Usage,
}

/// Default redb path under XDG_DATA_HOME (per B-decision B5).
fn default_db_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("chronos").join("chronos.db");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("chronos")
                .join("chronos.db");
        }
    }
    // Last-resort fallback: CWD. Should never hit in practice.
    PathBuf::from("chronos.db")
}

/// Expand a leading `~` in a user-provided path to the home directory.
///
/// Per R-tilde-expansion (m8-04 scoping doc §3): clap does this for free,
/// but our hand-rolled parser must do it manually. Only the literal `~`
/// and `~/...` forms are supported — `~user/...` is intentionally NOT
/// supported (would require reading /etc/passwd and is out of scope for
/// an operator tool).
fn expand_tilde(input: &str) -> PathBuf {
    if input == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
        return PathBuf::from(input);
    }
    if let Some(rest) = input.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(input)
}

/// Parse argv (excluding the program name) into a [`Command`].
///
/// `argv` is expected to be `std::env::args().skip(1)`.
pub fn parse(argv: &[String]) -> Result<Command, ArgsError> {
    if argv.is_empty() {
        return Ok(Command::Help);
    }
    // Pre-scan for `--db <path>` so it can appear before or after the subcommand.
    let mut db: Option<PathBuf> = None;
    let mut positional: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        match arg.as_str() {
            "--help" | "-h" => return Ok(Command::Help),
            "--db" => {
                let next = argv.get(i + 1).ok_or(ArgsError::MissingDbPath)?;
                db = Some(expand_tilde(next));
                i += 2;
            }
            _ => {
                positional.push(arg.as_str());
                i += 1;
            }
        }
    }
    let db = db.unwrap_or_else(default_db_path);

    match positional.as_slice() {
        ["test", "replay", bundle_id] => Ok(Command::Replay {
            bundle_id: (*bundle_id).to_string(),
            db,
        }),
        ["test", "replay"] => Err(ArgsError::MissingArgument("<bundle_id>".into())),
        // `test run` with no program args is captured by the `rest @ ..` arm
        // with `rest` being empty. Single arm covers both cases.
        ["test", "run", rest @ ..] => Ok(Command::Run {
            db,
            args: rest.iter().map(|s| (*s).to_string()).collect(),
        }),
        [] => Ok(Command::Help),
        _ => Err(ArgsError::Usage),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn empty_argv_is_help() {
        let argv: Vec<String> = vec![];
        assert_eq!(parse(&argv).unwrap(), Command::Help);
    }

    #[test]
    fn explicit_help_flag() {
        assert_eq!(parse(&args(&["--help"])).unwrap(), Command::Help);
    }

    #[test]
    fn replay_subcommand() {
        match parse(&args(&["test", "replay", "bundle-123"])).unwrap() {
            Command::Replay { bundle_id, db } => {
                assert_eq!(bundle_id, "bundle-123");
                assert!(db.ends_with("chronos.db"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn replay_requires_bundle_id() {
        let err = parse(&args(&["test", "replay"])).unwrap_err();
        match err {
            ArgsError::MissingArgument(_) => {}
            e => panic!("unexpected: {e:?}"),
        }
    }

    #[test]
    fn replay_with_db_flag() {
        match parse(&args(&["--db", "/tmp/chrono.db", "test", "replay", "b1"])).unwrap() {
            Command::Replay { bundle_id, db } => {
                assert_eq!(bundle_id, "b1");
                assert_eq!(db, PathBuf::from("/tmp/chrono.db"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn run_subcommand_captures_rest() {
        match parse(&args(&["test", "run", "./hello", "--arg"])).unwrap() {
            Command::Run { args, .. } => {
                assert_eq!(args, vec!["./hello".to_string(), "--arg".to_string()]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn run_subcommand_no_args_is_ok() {
        match parse(&args(&["test", "run"])).unwrap() {
            Command::Run { args, .. } => assert!(args.is_empty()),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn tilde_expansion_on_db() {
        // SAFETY: tests run in parallel but HOME is process-wide; the value
        // we set is only observed by tests in this module (no other env
        // mutations), and we restore the prior value at the end.
        let prior = std::env::var("HOME").ok();
        std::env::set_var("HOME", "/home/alice");
        match parse(&args(&["--db", "~/data.db", "test", "replay", "x"])).unwrap() {
            Command::Replay { db, .. } => {
                assert_eq!(db, PathBuf::from("/home/alice/data.db"));
            }
            other => panic!("unexpected: {other:?}"),
        }
        // Bare `~` resolves to $HOME.
        match parse(&args(&["--db", "~", "test", "replay", "y"])).unwrap() {
            Command::Replay { db, .. } => assert_eq!(db, PathBuf::from("/home/alice")),
            other => panic!("unexpected: {other:?}"),
        }
        match prior {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    fn unknown_subcommand_errors() {
        // A bare "test" with neither "run" nor "replay" falls through to Usage.
        let err = parse(&args(&["test"])).unwrap_err();
        assert!(matches!(err, ArgsError::Usage));
    }
}
