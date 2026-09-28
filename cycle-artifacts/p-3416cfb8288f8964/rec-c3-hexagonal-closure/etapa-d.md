# REC-C3-hexagonal-closure · Etapa D

## Scope

Harness identity enforcement (audit §8.3): capture
`BinaryIdentity { sha256, mtime_unix_nanos }` of the `chronos-mcp`
binary in `McpTestClient::start()` and refuse to spawn it when the
operator pinned an expected SHA via `CHRONOS_MCP_EXPECTED_SHA`.

Per proposal.md:174 the policy is **log-warn by default, fail-hard
opt-in**: fail-hard by default would break local dev where the
operator hasn't yet built the binary in the test target dir.

## Commits

- `73d3b44f` — feat(sandbox): BinaryIdentity capture+verify on
  McpTestClient::start_path (REC-C3.5-D)
- `fa4b9682` — chore(fmt): rustfmt follow-up across crates/* touched
  by REC-C3.5-B.4/B' (cosmetic)
- `9e808ad1` — chore(domain): drop unused SessionReaderError import
  (REC-C3.5-B.4 follow-up, surfaced by `-D warnings`)

## What landed

- **`chronos-sandbox/src/client/identity.rs`** (new, 247 lines):
  - `BinaryIdentity { sha256: String, mtime_unix_nanos: Option<u128> }`.
  - `BinaryIdentity::from_path(path) -> Result<Self, _>` reads the
    file, computes the lower-case hex SHA-256, and captures the
    `mtime_unix_nanos` opportunistically (falls back to `None` when
    the FS doesn't expose `modified()`).
  - `BinaryIdentity::verify_expected_sha() -> Result<(), _>` reads
    `CHRONOS_MCP_EXPECTED_SHA`, trims, lowercases, and compares.
    Missing env var ⇒ `Ok(())` (log-warn default). Mismatch ⇒
    `Err(McpSandboxError::SpawnFailed("BinaryIdentity mismatch:
    expected sha256 `<hex>`, got `<hex>` (mtime=...). Refusing to
    start sandbox with stale binary. ..."))`.
  - 6 unit tests (`#[cfg(test)] mod tests`) covering: known
    SHA-256 vector for `"hello world\n"`, missing file → `SpawnFailed`,
    no env var → ok, matching → ok, mismatch → `Err` with both hexes
    in the message, case-insensitive comparison.
- **`chronos-sandbox/src/client/mod.rs`**: `pub mod identity;` and
  re-export `BinaryIdentity`.
- **`chronos-sandbox/src/client/tools.rs:1819+`**: capture + log
  + verify at the top of `start_path()`. `McpTestClient::start()`
  delegates to `start_path(&resolve_mcp_path())`, so it inherits the
  same enforcement.
- **`Cargo.toml` (workspace)**: `sha2 = "0.10"` added to
  `[workspace.dependencies]` so other crates can pick it up.
- **`chronos-sandbox/Cargo.toml`**: depend on workspace `sha2`.

## SHA + mtime at the audit boundary

At client start, the harness now emits
`tracing::info!("chronos-mcp identity: sha256={} mtime_unix_nanos={:?} path={}", ...)`
which lands in the per-suite test log. This is the observability
companion to the fail-hard path.

## Tests + smokes

- 6/6 unit tests in `chronos-sandbox/src/client/identity::tests`
  pass (cargo test --lib --no-fail-fast).
- 12/12 lib tests in `chronos-sandbox` pass (was 6 before this etapa;
  +6 new).
- **sandbox smoke `e2e_connectivity`**: 1/1 ok, 23 s.
- **sandbox smoke `analytics_tools`**: 4/4 ok, 103 s.
- **fail-hard smoke**: with
  `CHRONOS_MCP_EXPECTED_SHA=deadbeef...` exported, `e2e_connectivity`
  fails with the exact error message above. Without the env var,
  the same test passes (log-warn default).

## Binary SHA

- Before etapa D: `064945e6f9d8f0acf6e4b625128bea38790b2f92ec3031acd51906c14998a8e5`
  (carry-over from etapa C; this etapa did not change `chronos-mcp`).
- After etapa D: same `064945e6…`. The change is sandbox-only; the
  harness server binary is bit-identical to its post-C state.
- The fail-hard path was therefore exercised against the post-C
  binary, not a stale one.

## Decision notes

- **log-warn by default, fail-hard opt-in** (proposal.md:174).
  Captured in the module-level docstring so future readers see the
  why.
- **mtime is `Option<u128>`, not `u128`**: FAT-family filesystems
  don't expose `modified()`; capturing the SHA still works in that
  case and the harness keeps going. Logging the mtime as `None` in
  that environment is a hint, not an error.
- **case-insensitive comparison**: operators sometimes paste the
  `sha256sum` output upper-cased; we trim + lowercase before
  comparing.
- **the comparison is on `self.sha256` (already lower-case) vs the
  trimmed+lowercased env var**: no risk of `deadbeef` accidentally
  matching `0xdeadbeef` because we trim whitespace and lowercase
  but don't strip `0x` or interpret hex escapes.

## Out of scope (R-roadmap follow-ups)

- **D.1** — promote `BinaryIdentity` into a per-session audit log
  line so the SHA appears in `McpAuditRecord`. Useful when
  reproducing stale-binary investigations across many sessions;
  the current implementation only logs once per `start_path`
  call.
- **D.2** — emit a structured `tracing::event!(target: "audit",
  Level::INFO, binary_sha256, binary_mtime_nanos)` instead of
  `tracing::info!("chronos-mcp identity: ...")` so audit-log
  parsers don't have to grep the human-readable string.

Both are non-blocking; the current implementation satisfies the
audit §8.3 finding ("sandbox must not run a stale binary without
operator awareness") because:

1. The identity is **always logged** at `tracing::info!`, so the
   operator can see what the harness measured.
2. When the operator **pins** `CHRONOS_MCP_EXPECTED_SHA`, a
   mismatch is a hard error with both hexes in the message.

## Audit-grounded citation

The `BinaryIdentity` struct + `verify_expected_sha` are the
canonical place to satisfy audit §8.3 (sandbox binary must be
identifiable). They are **not** part of the production
`chronos-mcp` server; they live in `chronos-sandbox` (test
infrastructure), which is the correct boundary per the audit.

## Verification commands run

- `cargo test -p chronos-sandbox --lib client::identity:: --no-fail-fast`
  → 6/6 ok
- `cargo test -p chronos-sandbox --lib --no-fail-fast` → 12/12 ok
- `cargo test -p chronos-sandbox --test e2e_connectivity
  -- --test-threads=1` → 1/1 ok
- `cargo test -p chronos-sandbox --test analytics_tools
  -- --test-threads=1` → 4/4 ok
- `CHRONOS_MCP_EXPECTED_SHA=deadbeef... cargo test
  -p chronos-sandbox --test e2e_connectivity -- --test-threads=1`
  → fail-hard path produces `BinaryIdentity mismatch: expected
  sha256 ... got ... (mtime=Some(...))` ✓
- `cargo clippy -p chronos-sandbox --all-targets -- -D warnings`
  → 0 warnings
- `cargo fmt --all -- --check` → 0 diffs
