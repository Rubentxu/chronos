# AGENTS.md

Operating rules for any agent (human or AI) working in this repository. Read
this before touching code.

This file replaces ad-hoc commands. Its purpose is to make every cycle cheap,
predictable, and proportional to the scope of the change.

---

## 1. Test topology (one-time audit)

The repository has five distinct test buckets. Knowing which one to run is
the single most important decision.

| Bucket | What lives here | Speed | Count today | When to run |
|---|---|---|---|---|
| **A — lib unit** | `#[test]` / `#[tokio::test]` inside `crates/*/src/` | <1 s each | **634 tests** | Default inner loop |
| **B — per-crate integration** | `crates/<c>/tests/*.rs` (16 files: browser, e2e, ebpf, go, java, js, python, …) | 1-5 s each | 16 files | When the changed crate exposes public API used by integration |
| **C — chronos-sandbox** | `chronos-sandbox/tests/*.rs` (35 files; **34 of them spawn `chronos-mcp` as a subprocess**) | 5-100 s each | 35 files, ~10-30 min full | Release candidates only |
| **D — chronos-e2e** | `crates/chronos-e2e/tests/*.rs` (1 file today: ptrace capture) | variable | 1 file | Explicit opt-in, needs root + ptrace kernel |
| **E — benches** | `crates/chronos-query/benches/`, `crates/chronos-store/benches/` | minutes | 2 | Opt-in on perf changes |

`chronos-sandbox` integration tests need a pre-built server. They look up the
binary in this order (`chronos-sandbox/src/client/tools.rs::McpTestClient::start`):

1. `CHRONOS_MCP_PATH` env var
2. `CARGO_BIN_EXE_chronos-mcp` env var (set by `cargo test` when a dev-dep on
   the binary exists)
3. `../../target/debug/chronos-mcp` relative to the test exe
4. Bare `chronos-mcp` in `$PATH`

If (3) does not exist because `CARGO_TARGET_DIR` is somewhere else, sandbox
tests fail with `SpawnFailed("No such file or directory")` and **every single
one panics in fixture setup**. Symptom: hundreds of failures, all in
`McpTestClient::start`. Fix: build the binary first and export the path:

```bash
cargo build --bin chronos-mcp
export CHRONOS_MCP_PATH="$CARGO_TARGET_DIR/debug/chronos-mcp"
```

---

## 2. Tiered test gates (the rule)

Run **only the buckets you need**. Match the tier to the SDDK path.

| SDDK path | Scope | Tier required | What to run |
|---|---|---|---|
| **B-direct** | Trivial, single crate, no probe/mcp touched | T0 + T1 | fmt + clippy + `--lib` of changed crate |
| **A-min** | 1-3 crates, no architectural fork | T0 + T2 | T1 + integration of changed crates |
| **A-lite** | Bounded, cross-cutting | T0 + T2 + T4-smoke | T2 + 1-3 representative sandbox suites |
| **A-full** | Architectural / new domain | T0 + T3 + T5 | All lib + per-crate integration + full sandbox |
| Pre-archive / release | Anything | T0 + T3 + T5 | Same as A-full |

| Tier | Command (with `CARGO_TARGET_DIR` honored) | Wall time on this repo |
|---|---|---|
| **T0 — lint gate** | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | ~30 s |
| **T1 — lib unit only** | `cargo test --workspace --lib --no-fail-fast` | ~5-10 s |
| **T2 — unit + per-crate integration of changed crates** | `cargo test -p <c1> -p <c2> … --tests --no-fail-fast` | ~10-30 s |
| **T3 — full unit + per-crate integration (no sandbox)** | `cargo test --workspace --lib --tests --exclude chronos-sandbox --no-fail-fast` | ~30-60 s |
| **T4-smoke — sandbox subset** | `cargo test -p chronos-sandbox --test <one> --test <two>` (pick probes that touch changed code) | ~1-3 min |
| **T5 — full sandbox** | `cargo test -p chronos-sandbox --no-fail-fast -- --test-threads=1` | ~10-30 min |

When the cycle changes probe/mcp plumbing (anything that affects how sandbox
spawns or talks to the server), **T4-smoke is mandatory before merge** even on
A-min. Sandbox tests are the only signal that exercises the full client ↔
server round-trip.

### How to pick the sandbox subset for T4-smoke

1. Run `git diff --name-only main..HEAD` on the cycle branch.
2. Look at the touched file paths and the affected MCP tool surface.
3. Pick 2-4 sandbox suites whose names match:
   - `analytics_tools.rs`, `boundary_conditions.rs` — broad coverage, slow
   - `e2e_connectivity.rs` — confirms server starts
   - `program_scenarios.rs` — exercises probe lifecycle on real binaries
   - `<topic>_tools.rs` — if your change adds/modifies an MCP tool
4. Document the chosen subset in the apply-checkpoint under
   `notes[].smoke_subset`.

If unsure, run `e2e_connectivity` + `analytics_tools` — they cover start,
drain, stop, and basic analytics.

---

## 3. Sandbox execution rules

- **Pre-build the binary before running any bucket C.** Do not let cargo
  spawn 35 individual test exes that each retry `cargo build --bin chronos-mcp`
  on first call. The cache exists; use it.
- **Always set `CHRONOS_MCP_PATH` explicitly** when `CARGO_TARGET_DIR` is not
  `target/`. This is the single most common cause of "all tests panic".
- **Default `--test-threads=1`** for sandbox runs: many tests share a fixture
  path and `tmpdir()` collisions can produce spooky failures. Lib/integration
  tests can use parallel threads; they are isolated by `cargo test`.
- **Do not pass `--no-fail-fast` to clippy** (it does not accept it well) and
  do **not** redirect `cargo test` output to `/dev/null` — you will lose the
  panic messages that distinguish "MCP not found" from a real bug.

### Background runs

Long T5 runs should go to background. Rules:

- Set `run_in_background: true` AND a `notify: true` flag (see jcode
  background-task docs).
- **Always poll at most every 30 s**, never sleep + tail. Use `bg status` or
  `bg wait` with a max_wait_seconds.
- Set `max_wait_seconds <= 600` (10 min) and re-`bg wait` if it elapses.
- Kill with `bg cancel <task_id>` if progress stalls (>2 min between
  `test result:` lines).
- A stalled sandbox run usually means: (a) a real test hangs (kill and
  inspect the partial log), (b) an MCP subprocess leaked (`pgrep -f
  chronos-mcp` and `kill`), or (c) the test is waiting on a `tokio::time::sleep`
  that is intentional. Distinguish by checking the most recent line in the
  log before killing.

---

## 4. Lint, fmt, and pre-existing breakage

The repository accumulated clippy drift on `main` before this file existed
(see commit history of `feat/m0-truth-first-foundation`). When you join an
unclean tree:

1. Run T0 first.
2. If it fails on a site you did not touch, decide:
   - **Fix in-cycle** (option A): apply the minimum patch, commit as
     `chore(clippy): fix pre-existing lints blocking -D warnings`, then re-run
     T0. This is the default.
   - **File a follow-up cycle** (option B): only if the cascade exceeds ~10
     sites or has behavior implications. Open a follow-up
     `m0-preflight-cleanup` (or appropriate) change.
3. Do **not** `#[allow(...)]` your way out of an unrelated lint cascade. The
   allow goes into the cycle, becomes noise for the reviewer, and is forgotten.

---

## 5. Branch / commit / cycle discipline

- **Trunk = `main`.** All cycles branch off `main`. Gate every cycle on
  `git fetch origin main && git checkout main && git pull --ff-only`.
- **One cycle = one branch.** `feat/m0-*` for foundation work,
  `feat/m<N>-*` for milestone N work, `fix/*` and `chore/*` for short-lived
  branches.
- **Commits are reviewable work units.** See `docs/manual-ai/01-core-pattern.md`
  for the convention. A clippy cascade across 13 crates deserves **one**
  `chore(clippy): …` commit, not 13.
- **Update `apply-checkpoint.json`** (in the SDDK vault) after every commit.
  Include:
  - Current `head_sha`
  - Commit list since base
  - `notes[]` array describing gate results, scope decisions, and any
    sandbox subset chosen for T4-smoke.

---

## 6. Forbidden patterns

- ❌ `cargo test --workspace --tests -- --test-threads=1` run in the foreground
  with a 50-minute budget. Use tiers.
- ❌ Editing `docs/propuestas/` (legacy, frozen by repo convention).
- ❌ Running `cargo fmt` without committing the diff.
- ❌ Bypassing clippy with module-level `#[allow(clippy::all)]`.
- ❌ Deleting or rewriting a test to make it pass without recording why in
  `apply-checkpoint.json`.
- ❌ Running the full sandbox suite as a smoke check on a docs-only cycle.
- ❌ Spawning `cargo test` without `CHRONOS_MCP_PATH` when sandbox is in
  scope.

---

## 6.5. Known pre-existing flakes

These tests fail intermittently on `main` without any cycle change. They are
tracked here so we do not chase them as regressions in every cycle:

| Test | Crate | When it flakes | Reproduction |
|---|---|---|---|
| `ptrace_tracer::tests::test_launch_with_syscall_tracing` | `chronos-native` | ~50% when run with full `cargo test --lib`; passes 3/3 in isolation | Same on `main` (c76b1096) and on every `feat/*` cycle |

If a new "flake" appears, first verify it reproduces on `main`:

```bash
git checkout main && cargo test -p <crate> --lib --no-fail-fast
```

If it does, treat it as pre-existing and file an M1+ follow-up. If it does
not, it is a real regression and must be fixed in the current cycle.

## 7. Quick reference

```bash
# Fastest useful gate (T0 only):
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# Unit tests for one crate (T1 / T2 minimal):
cargo test -p chronos-domain --lib

# Unit tests for the workspace, no sandbox (T3):
cargo test --workspace --lib --tests --exclude chronos-sandbox --no-fail-fast

# Sandbox smoke (T4) — set the binary path first:
cargo build --bin chronos-mcp
export CHRONOS_MCP_PATH="${CARGO_TARGET_DIR:-target}/debug/chronos-mcp"
cargo test -p chronos-sandbox --test e2e_connectivity --test analytics_tools

# Full sandbox (T5) — only on A-full or pre-archive:
cargo test -p chronos-sandbox --no-fail-fast -- --test-threads=1

# Build-only check (compile but do not run; use when you suspect breakage):
cargo test --workspace --lib --tests --no-run
```

If a command stalls or its output looks the same on two consecutive polls,
kill it, inspect the partial log, and re-classify the cycle scope before
re-running.
