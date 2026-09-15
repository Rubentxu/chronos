# Session handoff — 2026-09-15 (m10-ms-cap-discovery-followup closed)

## Status of the previous carry-over

The handoff from `HANDOFF-2026-09-15-session-close.md` (commit `9bdcda01`)
named **MS-CAP-DISCOVERY-FOLLOWUP** as the bounded A-min cycle that would
clean up the two MEDIUM debt findings introduced by `m10-ms-cap-discovery`
(`overeng-001` and `coupling-001`).

This cycle is **CLOSED** at the start of this session.

## Cycle m10-ms-cap-discovery-followup — outcome

- **Path**: A-min
- **Branch**: `feat/m10-ms-cap-discovery-followup`
- **Tag**: `v0.7.104` (annotated, on the merge commit)
- **Base SHA**: `9bdcda01be3a3c9334bc27e54466ca97fb3939d8`
- **Head SHA (merge to main)**: `11efb266bc10dd21f48e606b403740cefaeb6da2`
- **Feature commit**: `4bb10eff feat(chronos-mcp): MS-CAP-DISCOVERY-FOLLOWUP — toolset_guard helper + ALL_TOOL_NAMES sync assertion (closes FIND-DEBT-001, FIND-DEBT-002)`
- **Workspace cycle status**: `CLOSED`, `phase: archive`, `runtime_summary.remediating: false`.

### What shipped

One Rust file, 147 insertions / 51 deletions (net +96 LOC):
`crates/chronos-mcp/src/server.rs`:
- New private `fn toolset_guard(&self, tool_name: &str) -> Option<CallToolResult>`
  adjacent to `is_tool_listed`, single source of truth for the toolset-guard
  error message (REQ-CAP-008).
- 10 byte-identical inline guard blocks at dispatch handlers replaced with
  3-line `if let Some(err) = self.toolset_guard("<name>") { return Ok(err); }`
  call-sites. (`grep -c 'REQ-CAP-005 fallback' server.rs` → 0;
  `grep -c 'self.toolset_guard(' server.rs` → 10.)
- New `#[cfg(test)] mod toolset_sync_check` block with two `#[test] fn`s
  (`all_tool_names_covers_registrations`, `all_tool_names_have_registration`)
  and a hand-maintained `REGISTERED_TOOLS` const (61 entries) that
  asserts the `ALL_TOOL_NAMES ↔ #[tool]` registrations alignment
  (REQ-CAP-009).

### Verifications

- `verify-report.md`
  (`sha256:10222d5e906ff3f650f183b3e0c53ce26a587a87a344cf83a5ad92718f8f6b59`)
  verdict **PASS_WITH_WARNINGS** — REQ-CAP-008/009/010 all PASS;
  1 docstring wording nit (`const _` vs `#[test] fn`) closed at archive.
- Test totals: 99 mcp lib + 49 mcp integration + 272 services lib + 2 services integration = **422 tests, 0 failures**. Net +43 from the prior 379 baseline.
- Both prior MEDIUM debt findings (`overeng-001`, `coupling-001`)
  moved to **Terminated** in `terms/index.md`.

### Specs synced into the vault

- `specs/capabilities-discovery/spec.md` — appended REQ-CAP-008/009/010 sections.
- `specs/capabilities-discovery/REQ-ToolsetGuardSingleSource.md` — NEW (REQ-CAP-008).
- `specs/capabilities-discovery/REQ-AllToolNamesBuildAssertion.md` — NEW (REQ-CAP-009) + one-line implementation note appended at archive (`#[cfg(test)] #[test] fn` shape, not `const _ : () = {...}` because stable Rust const-eval is restrictive).
- `specs/capabilities-discovery/REQ-IsToolListedStability.md` — NEW (REQ-CAP-010).

### Vault validation at archive time

`sddk vault validate` returned exit 1, 125 nodes, 58 errors (all pre-existing — 0 introduced by this cycle). Pre-existing errors fall into two buckets:
- VAULT002 (duplicate node ids) across m0/m2/m8 cycles.
- VAULT003 (missing wikilinks from old REQs) across m0/m2/m8 cycles.

### Carry-forward (out of this cycle's scope; both from REQ-CAP-009 in archive-report §"Carry-forward")

- **REQ-CAP-011 candidate** — derive `ALL_TOOL_NAMES` from `ServerHandler::list_tools()` once rmcp exposes an owned/sync form (currently borrowed-lifetime).
- **REQ-CAP-012 candidate** — bidirectional sync (catch deletions too) via `LinkMap<&'static str, ()>` at each `#[tool]` site.

### Operational note

The default server behaviour is byte-identical to the previous release
(`v0.7.103` → `v0.7.104`). No public API change. No wire-shape change.
The only operator-visible difference is what would happen if a future
cycle adds a `#[tool(name = "x")]` and forgets to extend `ALL_TOOL_NAMES`:
`cargo test -p chronos-mcp` will now fail with
`Tool \`x\` is declared via #[tool] but missing from ALL_TOOL_NAMES.`,
instead of silently dropping the new tool's availability record.

## Pointer for the next session

Two natural follow-ups remain for m10:

1. **MS-CAP-DISCOVERY-FOLLOWUP-2** (A-min or A-lite): address REQ-CAP-011
   (derive `ALL_TOOL_NAMES` from `list_tools()`) once a usable rmcp
   version lands. Until then, the `toolset_sync_check` module is
   sufficient.
2. **m10-spec-coverage-glue** (A-full): reconcile the new
   `capabilities-discovery` REQ set (now REQ-CAP-001 through REQ-CAP-010)
   with the agent API v2 surface, and chase any leftover m10 documentation
   drift. The `cycles/index.md` row in the durable vault now lists two
   closed m10 cycles (`v0.7.103`, `v0.7.104`), and this is the natural
   point to close the m10 m-series with a sync-of-syncs.

If neither feels urgent, the next-debt cycle that has runtime evidence
will pick up the cycle (m0/m2/m8 vault hygiene cycles, or a new
backlog item from a fresh verify report on a fresh change).

Until the user picks, leave the workspace on `main` at `11efb266bc…da2`,
cycle `p-3416cfb8288f8964/m10-ms-cap-discovery-followup` left CLOSED, and
the `feat/m10-ms-cap-discovery-followup` branch intact (archive rule: do
not delete branch before the manifest is persisted).
