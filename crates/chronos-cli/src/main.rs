//! `chronos` CLI binary — operator entry point for Chronos.
//!
//! Subcommands (m8-04):
//!
//! - `chronos test replay <bundle_id>` — re-run a persisted counterexample
//!   bundle against the events captured at shrink time. Working today.
//! - `chronos test run` — start a live probe against a target program.
//!   Stub (m9+).
//!
//! B-decision B2 (m8-04 scoping doc): the CLI dispatches directly into the
//! services / store layer — it does NOT open a JSON-RPC channel to the
//! chronos-mcp server. This keeps the CLI dependency-free at runtime
//! (no need for `chronos-mcp` running) and avoids round-tripping through
//! a separate process for what is fundamentally a local-disk read + in-process
//! computation.

use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;

mod args;
mod replay;
mod run;

use args::{parse, ArgsError, Command};

const USAGE: &str = "\
Usage:
  chronos [--db <path>] test replay <bundle_id>
  chronos [--db <path>] test run    <program-args...>
  chronos --help

Flags:
  --db <path>   redb session store path (default: $XDG_DATA_HOME/chronos/chronos.db)
  --help, -h    print this message

m8-04: `test replay` is fully implemented; `test run` is a stub (m9+).";

#[tokio::main]
async fn main() {
    init_tracing();
    if let Err(err) = run().await {
        // Top-level error funnel: print chain, exit non-zero.
        eprintln!("chronos: {err:#}");
        std::process::exit(1);
    }
}

fn init_tracing() {
    // Honor RUST_LOG; default to "info" so operators get one line per major step
    // without drowning in debug noise. tracing-subscriber is cheap; no JSON
    // formatter needed for an operator CLI.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

async fn run() -> Result<()> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let cmd = match parse(&argv) {
        Ok(cmd) => cmd,
        Err(err) => match err {
            ArgsError::Usage => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
            other => return Err(other.into()),
        },
    };

    match cmd {
        Command::Help => {
            println!("{USAGE}");
            Ok(())
        }
        Command::Replay { bundle_id, db } => {
            let report = replay::run_replay(&db, &bundle_id)
                .await
                .with_context(|| format!("replay bundle {bundle_id:?}"))?;
            // Pretty-print to stdout. Operators can pipe to `jq` since this is
            // pure JSON.
            println!(
                "{}",
                serde_json::to_string_pretty(&report).context("serialize replay report")?
            );
            Ok(())
        }
        Command::Run { db, args } => {
            // The stub takes the db path for symmetry with `replay`; it isn't
            // opened today but will be in m9+ when live-probe plumbing lands.
            let _ = &db;
            run::run_live_probe_stub(&db, &args)
        }
    }
}
