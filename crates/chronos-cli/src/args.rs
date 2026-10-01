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
    /// Run the live probe against a target program (STUB).
    ///
    /// Returns an operator-facing error naming the three live-probe runtime
    /// dependencies that are not landed (see `run::run_live_probe_stub`), so
    /// operators get a sane failure instead of a cryptic "command not found".
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

    /// Serialises every test in this module that reads or writes the
    /// process-wide variables `default_db_path` and `expand_tilde` consult.
    ///
    /// The harness runs tests on parallel threads, and both of those helpers
    /// read `XDG_DATA_HOME` / `HOME` from the environment rather than from
    /// parameters. Without this lock a test that pins a value is observed by
    /// an unrelated test running concurrently. Readers need the lock as well
    /// as writers: mutual exclusion only holds if both sides take it.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Holds [`ENV_LOCK`] and restores `XDG_DATA_HOME` / `HOME` on drop.
    ///
    /// Restoring at the end of the test body is not sufficient: a failing
    /// assert panics, unwinds past the restore, and leaves the environment
    /// corrupted for the remainder of the run. `Drop` runs in both cases.
    struct EnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        prior: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn acquire() -> Self {
            let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let prior = ["XDG_DATA_HOME", "HOME"]
                .into_iter()
                .map(|key| (key, std::env::var(key).ok()))
                .collect();
            Self { _lock: lock, prior }
        }

        fn set(&self, key: &str, value: &str) {
            std::env::set_var(key, value);
        }

        fn unset(&self, key: &str) {
            std::env::remove_var(key);
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.prior {
                match value {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
        }
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
        let _env = EnvGuard::acquire();
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
        let _env = EnvGuard::acquire();
        let err = parse(&args(&["test", "replay"])).unwrap_err();
        match err {
            ArgsError::MissingArgument(_) => {}
            e => panic!("unexpected: {e:?}"),
        }
    }

    #[test]
    fn replay_with_db_flag() {
        let _env = EnvGuard::acquire();
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
        let _env = EnvGuard::acquire();
        match parse(&args(&["test", "run", "./hello", "--arg"])).unwrap() {
            Command::Run { args, .. } => {
                assert_eq!(args, vec!["./hello".to_string(), "--arg".to_string()]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn run_subcommand_no_args_is_ok() {
        let _env = EnvGuard::acquire();
        match parse(&args(&["test", "run"])).unwrap() {
            Command::Run { args, .. } => assert!(args.is_empty()),
            other => panic!("unexpected: {other:?}"),
        }
    }

    // -----------------------------------------------------------------
    // B-decision B5: the default store path. Nothing pinned these four
    // branches — `replay_subcommand` only asserted `db.ends_with(
    // "chronos.db")`, which every branch satisfies, so reordering them
    // or dropping a path segment left the whole suite green.
    // -----------------------------------------------------------------

    #[test]
    fn default_db_path_prefers_xdg_over_home() {
        let env = EnvGuard::acquire();
        env.set("XDG_DATA_HOME", "/xdg");
        env.set("HOME", "/home/alice");
        assert_eq!(
            default_db_path(),
            PathBuf::from("/xdg/chronos/chronos.db"),
            "B5: a non-empty XDG_DATA_HOME takes precedence over HOME"
        );
    }

    #[test]
    fn default_db_path_falls_back_to_home_when_xdg_unset() {
        let env = EnvGuard::acquire();
        env.unset("XDG_DATA_HOME");
        env.set("HOME", "/home/alice");
        assert_eq!(
            default_db_path(),
            PathBuf::from("/home/alice/.local/share/chronos/chronos.db"),
            "B5: without XDG_DATA_HOME the path is XDG-data under HOME"
        );
    }

    #[test]
    fn default_db_path_treats_empty_xdg_as_unset() {
        let env = EnvGuard::acquire();
        // An exported-but-empty XDG_DATA_HOME is how a shell that
        // interpolates an unset variable presents itself. Treating it as a
        // real root would yield the relative path "/chronos/chronos.db".
        env.set("XDG_DATA_HOME", "");
        env.set("HOME", "/home/alice");
        assert_eq!(
            default_db_path(),
            PathBuf::from("/home/alice/.local/share/chronos/chronos.db"),
            "an empty XDG_DATA_HOME must be treated as unset, not as a root"
        );
    }

    #[test]
    fn default_db_path_falls_back_to_cwd_without_any_root() {
        let env = EnvGuard::acquire();
        env.unset("XDG_DATA_HOME");
        env.set("HOME", "");
        assert_eq!(
            default_db_path(),
            PathBuf::from("chronos.db"),
            "with neither XDG_DATA_HOME nor HOME the path is CWD-relative"
        );
    }

    #[test]
    fn tilde_expansion_on_db() {
        let env = EnvGuard::acquire();
        env.set("HOME", "/home/alice");
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
    }

    #[test]
    fn tilde_stays_literal_without_home() {
        let env = EnvGuard::acquire();
        env.unset("HOME");
        assert_eq!(
            expand_tilde("~"),
            PathBuf::from("~"),
            "with no HOME a bare `~` must stay literal rather than resolve to \"\""
        );
        assert_eq!(
            expand_tilde("~/data.db"),
            PathBuf::from("~/data.db"),
            "with no HOME a `~/` path must stay literal rather than resolve to \"/data.db\""
        );
    }

    #[test]
    fn tilde_user_form_is_not_expanded() {
        // `~user/...` is intentionally out of scope (see `expand_tilde`).
        // The input never matches the `~` or `~/` forms, so this reads no
        // environment and needs no guard.
        assert_eq!(
            expand_tilde("~root/data.db"),
            PathBuf::from("~root/data.db")
        );
    }

    #[test]
    fn unknown_subcommand_errors() {
        let _env = EnvGuard::acquire();
        // A bare "test" with neither "run" nor "replay" falls through to Usage.
        let err = parse(&args(&["test"])).unwrap_err();
        assert!(matches!(err, ArgsError::Usage));
    }
}
