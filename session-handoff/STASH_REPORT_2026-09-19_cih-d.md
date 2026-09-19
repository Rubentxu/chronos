# Stash report — cih-d-stash-non-mine (object f97b5798)

> Independent report on the unstaged work found in the worktree before
> the CIH-D translator fix was committed. The work was preserved in
> `git stash` as `stash@{0}` (object SHA
> `f97b5798ac93791483b7e0468fab52989b8b90a0`), parented at commit
> `6441116df383e960d01f751fe7f5d8179d42793a` (the pre-CIH-D HEAD of
> `rec-c3-ci-hygiene`). This report is informational; no action is
> taken on the stash without explicit operator decision.

## Identification

| Property | Value |
|---|---|
| Stash ref | `stash@{0}` |
| Stash message | `cih-d-stash-non-mine` (msg from `git stash push -m ...`) |
| Object SHA (commit) | `f97b5798ac93791483b7e0468fab52989b8b90a0` |
| Parent (HEAD at stash time) | `6441116df383e960d01f751fe7f5d8179d42793a` |
| Branch context | `(no branch)` — stash was made from a detached HEAD |
| Files touched | 3 (190 deletions, 33 insertions net) |

## Exact diff (against `stash@{0}^1` = `6441116d`)

### `chronos-sandbox/src/client/process.rs` — net `-31 / +0`

**What it does**: removes the CIH-B instrumentation on `McpProcess::spawn`.
Before (CIH-B): two distinct failure modes (`spawn()` returns `Err`,
or `try_wait()` shows the child already exited) both surface as
`McpSandboxError::SpawnFailed` with the executable path and exit status
inline. The "instrument the harness failure" comment in the prior code
explicitly cites CIH-B and the path is "included verbatim so the log
line is greppable".

The stash diff replaces that block with a flat
`.map_err(|e| McpSandboxError::SpawnFailed(e.to_string()))?` that
loses the executable path, the cwd, and the post-spawn `try_wait()`
guard. The result: a bare `e.to_string()` propagates back to the test
fixture, which is exactly the `SpawnFailed("No such file or directory
(os error 2)")` message that CIH-B was added to disambiguate.

### `chronos-sandbox/src/client/tools.rs` — net `-157 / +33`

**What it does**: reverts `McpTestClient::resolve_mcp_path` (and the
helper functions `resolve_target_dir`, `cargo_metadata_target_dir`,
`which_in_path`, `build_chronos_mcp_via_cargo`, `workspace_root`,
plus the `OnceLock` cache) to a pre-CIH-B implementation. The new
implementation:

- Tries `CHRONOS_MCP_PATH` and `CARGO_BIN_EXE_chronos-mcp` (kept).
- Walks `current_exe().parent().parent().join("chronos-mcp")` — i.e.,
  assumes `CARGO_TARGET_DIR == "target"`. The CIH-B comment in the
  removed code explicitly called this out as the root cause of
  `m1_07`/`m1_08` silently falling back to PATH under tarpaulin.
- Has no `cargo metadata` fallback, no `which` walk, no lazy
  `cargo build` cache, and no workspace-root detection.

**Net effect**: any test that runs with a non-default `CARGO_TARGET_DIR`
(which is exactly the layout used by GH Actions runners that mount a
custom cache, AND by `cargo tarpaulin` with `--engine llvm` in some
configurations) will fall back to `SpawnFailed("No such file or
directory")` on the test fixture setup — masking every test in
`sandbox`-owned test files. This is the regression CIH-B fixed.

### `chronos-sandbox/tests/m1_acceptance.rs` — net `-34 / +33`

**What it does**: reverts `m1_02_execution_log_persistence_impl` Case 6
from the REC-C1.5.2 strict-replay contract to the legacy
"skip the first corrupt segment, recover from the second" path.

The stash diff:

- Drops the comment that quotes the REC-C1.5.2 contract and the
  `3cc511ef` anchor.
- Replaces the strict assertion (which expects
  `Err(LogError::ReplayIntegrity { kind: ReplayIntegrityError::CorruptSegment })`)
  with a permissive `SegmentedExecutionLog::open(...).expect("reopen")`
  followed by `assert_eq!(log2.tail_seq(), Some(EventSeq::new(2)))`.
- Removes the second-reopen reproducibility guard.

**Net effect**: the test no longer proves what its own header comment
claims. It accepts a `SegmentedExecutionLog` published with a
truncated BLAKE3 checksum on the retained region — i.e., it admits
exactly the failure mode that `commit 3cc511ef` froze out.

## Hypothesis of origin and intent

OBSERVED (from this session): the stash appeared in the worktree
*before* this session started working on CIH-D. It was present at
session resumption, already staged in the index (`git status` showed
`Changes to be committed:` for the three files). The pre-CIH-D HEAD
that the stash was parented on was `6441116d` (the "pre-integration
closure-pending fix" commit, i.e., the immediately preceding
main_sha=head_sha fix from the prior session).

OBSERVED (from `git stash show` output, prior to the stash being
recorded): the stash@{0} description verbatim was
`On (no branch): cih-d-stash-non-mine`. The `On (no branch)` confirms
the stash was made from a detached HEAD, which is consistent with the
worktree being in detached state at session start.

INFERENCE (not OBSERVED): the most likely origin is a prior agent (or
the operator) that was working on the same `rec-c3-ci-hygiene` branch,
intended to either:
  (a) revert CIH-B's harness fix and REC-C1.5.2 strict replay in
      order to "fix" a different problem (the leak between sandbox
      client and the lazy `cargo build` cache, perhaps), or
  (b) work around a flake by relaxing the test that depends on
      strict replay, or
  (c) pre-stage material for a "hygiene" pass that the prior session
      chose not to commit (this matches the staging status I observed
      on session resumption).

None of these hypotheses are confirmed. The stash message
`cih-d-stash-non-mine` was set by THIS session when I stashed the
files to isolate CIH-D — the prior stash message would have been
something else, but I did not record it.

**Hypothesis I cannot disprove**: a prior session may have intended
to commit these changes as part of a follow-up slice (e.g.,
rec-c3-ci-hygiene post-CIH-D followup), and the staging was its
intermediate state. The CIH-B comment in `tools.rs` and the REC-C1.5.2
comment in `m1_acceptance.rs` were deliberately removed, which is a
content-aware edit, not a blind revert — this suggests a deliberate
choice, not a transient artifact.

## Which contracts are violated by these changes (if applied)

| File | Contract | Evidence |
|---|---|---|
| `process.rs` | CIH-B evidence contract (`FIND-CIH-D-coverage-r2-stale-cursor-DrainFailed-mask` and the prior `FIND-CIH-B-spawn-failed-binary-resolution`): harness failures must carry executable path + cwd. | The stash diff removes the `executable={:?} cwd={:?} error={}` formatting and replaces it with a flat `e.to_string()`. |
| `tools.rs` | CIH-B harness contract: `resolve_mcp_path` must work for non-default `CARGO_TARGET_DIR` (e.g., `/var/home/rubentxu/cargo-targets/debug`, which is the layout used in this environment). | The stash diff removes `cargo metadata`, `cargo build --bin chronos-mcp` lazy fallback, `which`-style PATH walk, and `OnceLock` cache. Restores the `current_exe().parent().parent()` walk that only works when `CARGO_TARGET_DIR == "target"`. |
| `m1_acceptance.rs` | Architectural rule #6: "no modificar REC-C1.5.2 strict replay" (operator rule, locked). | The stash diff deletes the `Err(LogError::ReplayIntegrity { ... })` assertion, the second-reopen reproducibility guard, and the CIH-A atomicity comment. Replaces with permissive `.expect("reopen")` + `assert_eq!(log2.tail_seq(), Some(EventSeq::new(2)))`. |
| `m1_acceptance.rs` | Operator rule #4: no `#[ignore]`, no `--skip`, no waiver-on-test mechanisms. | Not a literal waiver, but the relaxed assertion is functionally equivalent: it accepts a `SegmentedExecutionLog` published with a corrupted retained segment. |
| `m9-75-fail-closed-store-open/archive-manifest.md` (Vault drift) | regen --check manifest SHA. | Tools.rs changes the file's SHA, which makes `regen_manifest_index_shas.py --check` fail on the m9-75 archive-manifest row for `chronos-sandbox/src/client/tools.rs` (verified during CIH-D local validation). |

## Recoverability (without violating REC-C1.5.2 or CIH-B)

### Fully recoverable (no contract violation)
- **process.rs**: **not recoverable without violating CIH-B**. The
  change is purely a regression of the harness evidence contract; the
  only way to "apply" it is to delete CIH-B's instrumentation. This is
  not a fix.

### Partially recoverable (one sub-feature, if any)
- **tools.rs**: the only defensible subset of the stash is the
  `CHRONOS_MCP_PATH` / `CARGO_BIN_EXE_chronos-mcp` env-var lookup
  block, which is preserved in both the pre- and post-CIH-B versions
  (the stash keeps it). It is NOT a separate change — it is the
  prefix of the reverted code. Recovering it would require reverting
  only the parts of `tools.rs` that delete CIH-B's added behavior,
  which is the entire 157-line deletion. There is no extractable
  sub-slice.

### Not recoverable (violates architectural rule)
- **m1_acceptance.rs**: **not recoverable under rule #6**. Case 6 is a
  regression guard for REC-C1.5.2 strict replay. Relaxing the
  assertion to "skip the first, recover from the second" is exactly
  the legacy lenient semantics that `3cc511ef` removed. This is a
  contract violation, not a fix.

## Proposed destinations (no action taken; operator decides)

1. **Discard the stash entirely** (`git stash drop stash@{0}`). This
   is the lowest-risk option if the changes have no owner and no
   intended destination. Risk: if a prior session meant to commit them
   as a separate architectural-decision slice, that intent is lost.

2. **Move the stash to a research branch** (`git stash branch
   research/cih-stash-investigation f97b5798`). Preserves the work
   without exposing it to the protected branch. Lets a future slice
   decide whether to revert (`git revert`) or salvage. Risk: the
   branch will accumulate drift and need a clear owner.

3. **Quarantine in a notes-only file** (this report). Leave the
   stash in `stash@{0}` with the report at
   `session-handoff/STASH_REPORT_2026-09-19_cih-d.md`. No code
   movement. The stash remains visible to future operators via
   `git stash list`.

**Recommendation**: option 3 (the current state). The operator's
rule #4 forbids waivers, rule #6 forbids modifying REC-C1.5.2, and
the CIH-B harness contract is the foundation of CIH-D's verification.
Until a separate architectural-decision cycle (NOT a hygiene cycle)
owns a deliberate reversal of CIH-B and REC-C1.5.2, the stash
should remain quarantined and unintegrated.

## Provenance

- Stash created: this session, `git stash push -m cih-d-stash-non-mine
  -- chronos-sandbox/src/client/process.rs chronos-sandbox/src/client/tools.rs
  chronos-sandbox/tests/m1_acceptance.rs`, run on 2026-09-19 around
  15:42 UTC, after the unstaged work was discovered during CIH-D
  pre-commit validation.
- Files were already modified and unstaged in the worktree at
  session start (the prior conversation summary mentioned a worktree
  with these changes unstaged at the end of the prior session, dated
  before this session).
- No destructive commands were run on the stash (no `stash drop`, no
  `reset --hard`, no `clean`).
- Verification that the stash is intact and unmodified:
  `git stash list` shows three entries (stash@{0}, stash@{1},
  stash@{2}) with `stash@{0}` matching the recorded SHA
  `f97b5798ac93791483b7e0468fab52989b8b90a0`.
