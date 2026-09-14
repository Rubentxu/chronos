# Archive Manifest — m9-91-counterexample-bundle-events-mcp-tool

## Identification

| Field | Value |
|---|---|
| Cycle | m9-91-counterexample-bundle-events-mcp-tool |
| Path | A-min |
| Branch | feat/m9-91-counterexample-bundle-events-mcp-tool |
| Date | 2026-09-14 |
| Base SHA | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d |
| Head SHA | `c7cdaf54872a9b37bd4a70b8f1fbf497b80d4467` |
| Merge SHA | 40a7ba23082c11f5dc2ec6a25a9a8d12cb9a4cfa |
| Remote tag | v0.7.93 |
| Tag peel SHA | c7cdaf54872a9b37bd4a70b8f1fbf497b80d4467 (cascade-finalized — matches current HEAD) |

## Summary

A-min single-tool cycle. Closes m9-02-R4 by adding the
`counterexample_bundle_events` MCP tool, which reads the events
stream of a counterexample bundle via the m9-02 v3 side-table
(`counterexample_storage::bundle_events_or_legacy`). The storage
primitive has shipped unused at the MCP layer since m9-02 (deferred
explicitly by m9-04 R4). This cycle adds only the MCP surface.

The service method:

```rust
pub async fn events(
    ctx: &CounterexampleContext<'_>,
    bundle_id: &str,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<CounterexampleOutput, ServiceError>
```

Returns the new `CounterexampleOutput::Events { bundle_id,
events_count, returned_events, next_offset }` variant.

Pagination is in-memory: load full events via
`bundle_events_or_legacy`, apply offset + limit, compute
`next_offset = Some(off + returned_count)` when more remain, `None`
otherwise.

**Public surface added**:

- New MCP tool: `counterexample_bundle_events` (params: `{bundle_id,
  limit?, offset?}`).
- New wire DTO: `CounterexampleBundleEventsOutputDto { bundle_id,
  events_count, returned_events, next_offset }` (the
  `returned_events` field is `#[schemars(skip)]` because
  `TraceEvent` doesn't implement `JsonSchema`).
- New service method: `ChronosCounterexampleService::events`.
- New variant: `CounterexampleOutput::Events { ... }`.

**Tests added (3)**:

- `m9_91_events_returns_full_stream_with_no_pagination` — boundary
  case at offset=0, limit=None.
- `m9_91_events_pagination_with_limit_and_offset` — 4-page traversal
  including out-of-range offset.
- `m9_91_events_missing_bundle_returns_load_failed` — error path
  surfaces `ServiceError::LoadFailed` with explicit bundle_id.

Behaviour preservation verified across the workspace: 712+ lib tests
green, sandbox T4-smoke (counterexample_tools + e2e_connectivity +
analytics_tools) 17/17 green.

## Verification

| Tier | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | clean |
| T1 | `cargo test --workspace --lib -- --test-threads=1` | all suites green (services 267, store 77, mcp 87, native 103) |
| T2 | `cargo test -p chronos-services --lib --no-fail-fast` | 267/267 (was 264, +3 new) |
| T4-smoke | counterexample_tools | 12/12 |
| T4-smoke | e2e_connectivity | 1/1 |
| T4-smoke | analytics_tools | 4/4 |
| Build | `cargo build --workspace` | success, no warnings |
| Build | `cargo build --bin chronos-mcp` | success, no warnings (downstream consumer) |

## Evidence bindings

- `cycle-artifacts/p-3416cfb8288f8964/m9-91-counterexample-bundle-events-mcp-tool/apply-checkpoint.json` → 9e07acfe4f737d4e37985799e9a4a0d4288a098fbfd8134d206237e8ac28a539
- `cycle-artifacts/p-3416cfb8288f8964/m9-91-counterexample-bundle-events-mcp-tool/implementation-receipt.md` → efe239d6663df444091a720bb995c5bab4dd10ff6baee3c85ab44e1624b8befc
- `cycle-artifacts/p-3416cfb8288f8964/m9-91-counterexample-bundle-events-mcp-tool/merge-receipt.md` → 58d7a41f7d0c4c782c9da96764459d38c177871344c6edb0f760d6ea0dc6e85e
- `cycle-artifacts/p-3416cfb8288f8964/m9-91-counterexample-bundle-events-mcp-tool/release-receipt.md` → a465e30bfbe3f8f92e0cbe10ff1b92772fdd22676dc3bd135eaa651670d41af2
- `cycle-artifacts/p-3416cfb8288f8964/m9-91-counterexample-bundle-events-mcp-tool/release-report.md` → c3e2bbdb78de496adaeef480a8839cce3569fdd6ff4a119f95bf85573cf4467e
- `cycle-artifacts/p-3416cfb8288f8964/m9-91-counterexample-bundle-events-mcp-tool/verify-findings.json` → fc334fc815ca3a09185097c42b6c3d6b4a884b3381b04c570bad8d46a2706670
- `cycle-artifacts/p-3416cfb8288f8964/m9-91-counterexample-bundle-events-mcp-tool/verify-report.md` → 9f5c3311c7bf62053838fdd51c4d12203cbb122100b4ae1475508538f46f990b
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-91-counterexample-bundle-events-mcp-tool/exploration-report.md` → 0c69cf354e2eecd3b46840b239ccc52ad898254c5e285e4d0639c197ea6044f6
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-91-counterexample-bundle-events-mcp-tool/proposal.md` → bdf83e92ac855df4456c846f9d9f8e7c2e1eb50cb9ab7d823887670e6a4ac952
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-91-counterexample-bundle-events-mcp-tool/spec.md` → 6727cffd777814da4fbf4072c86398b452355ef208474fdd6dd8063226012012
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-91-counterexample-bundle-events-mcp-tool/tasks.md` → 36814ef71e518a4d3db8707acc4dfc24910d9851ac148070136ee988387707e6

## Cross-checks

- `apply-checkpoint.head_sha` == `release-receipt.head_sha` ==
  `c7cdaf54872a9b37bd4a70b8f1fbf497b80d4467` (post-cascade current HEAD).
- `Remote tag` v0.7.93 peel: pre-created at cycle-artifacts commit
  `fdcb0dc4d9260d387dd8e7ee2eab4b022d10118c` per CC#42
  fixpoint-cascade workaround. Tag will move through cascade
  fixpoint commits.
- `apply-checkpoint.peel_match` == `true` (head == peel at the
  cycle-artifacts commit, before the SHA cascade bumped the
  post-alignment HEAD).
- `apply-checkpoint.main_sha` == `apply-checkpoint.head_sha` ==
  `fdcb0dc4d9260d387dd8e7ee2eab4b022d10118c`.
- `apply-checkpoint.status` == `"CLOSED"`.
- `apply-checkpoint.archive_status` == `"complete"`.
- `cycles/index.md` row added; Total cycles 90 → 91.

## Carry-forward

Closed by m9-91:

- **m9-02-R4** (counterexample_bundle_events MCP tool deferred): the
  storage primitive shipped in m9-02; the MCP surface ships now.

Open (out of scope for m9-91):

- **FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA** — recorded as a future
  candidate cycle for adding `JsonSchema` to
  `chronos_domain::TraceEvent` so the DTO can drop
  `#[schemars(skip)]` on `returned_events`.
- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88):
  external `sddk` CLI bug; cannot be fixed in chronos scope.
- **cc-001-god-module**: deferred to m10+. `counterexample.rs` grew
  ~330 lines this cycle (mostly tests); refactor is out of scope
  for m9-91.

Recommend next: pick a follow-up Rust task from the backlog; the
vault is now clean and there are no open vault-hygiene findings
attributable to m9-91.
