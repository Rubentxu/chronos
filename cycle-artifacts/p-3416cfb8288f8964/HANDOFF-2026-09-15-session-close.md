# Session handoff — 2026-09-15 (m10-ms-cap-discovery closed)

## Status of the previous carry-over

The handoff from 2026-09-14 (`HANDOFF-2026-09-14-session-close.md`,
commit `7cb19c38`) named **MS-CAP-DISCOVERY** as the next cycle and pointed at
`~/.sddk-knowledge/p-3416cfb8288f8964/specs/AGENT_API_V2/REQ-CapabilitiesToolAdditiveShape.md`
as the seed for the "what is in this server" capability discovery work
(A-min, branch `feat/m10-ms-cap-discovery`, ADR-0002).

This cycle is **CLOSED** at the start of this session.

## Cycle m10-ms-cap-discovery — outcome

- **Path**: A-min
- **Branch**: `feat/m10-ms-cap-discovery`
- **Tag**: `v0.7.103` (annotated, on the merge commit)
- **Base SHA**: `7cb19c3847aed54e19c985d377cde241520d4023`
- **Head SHA (merge to main)**: `3f7abc351974835a215767b9f491dad3aef3474e`
- **Feature commit**: `b1440d46 feat(chronos-mcp, chronos-services): MS-CAP-DISCOVERY — capabilities discovery toolset filtering`
- **Workspace cycle status**: `CLOSED`, `phase: archive`,
  `runtime_summary.remediating: false`.

### What shipped

Three Rust files, 849 insertions / 8 deletions:

1. `crates/chronos-services/src/output.rs` — additive `ToolAvailability` struct,
   plus `tool_availability: HashMap<String, ToolAvailability>` and
   `probed_at: Option<u64>` on both `CapabilitySnapshot` and
   `CapabilitiesOutput` (both `#[serde(default)]`, byte-identical wire shape
   for non-availability consumers).
2. `crates/chronos-services/src/session_lifecycle.rs` — updates `capabilities()`
   and `capabilities_with_context()` to thread through the new fields.
3. `crates/chronos-mcp/src/server.rs` — `ChronosServer::active_toolset` field,
   `ALL_TOOL_NAMES` / `NATIVE_TOOL_NAMES` / language toolset constants
   (≤25 native per REQ-CAP-005), `build_tool_availability()`,
   `is_tool_listed()`, REQ-CAP-005 pre-check guards on 10 language-dependent
   tools, `with_toolset()` test constructor.

Fourteen new tests (4 unit tests in chronos-services on the new snapshot shape,
10 in chronos-mcp covering the gating and the filtering). All T2 tests green
(`cargo test -p chronos-services -p chronos-mcp --lib --tests --no-fail-fast`):
272 services + 97 mcp + 14 new = 379 passing, 0 failures.

### Verifications

- `verify-report.md`
  (`sha256:266e976410c980f7fc1fafb9a4d27ecaa0257e706f7431377764155fd97a8b65`)
  verdict **PASS_WITH_WARNINGS** — 6 REQs compliant, 1 MODIFIED
  (`REQ-CapabilitiesToolAdditiveShape.md`); two warnings (tool count
  discrepancy between hand-listed `ALL_TOOL_NAMES` and `list_tools`, and
  pre-existing `chronos-native` ptrace flake excluded).
- `debt-report.json`
  (`sha256:9241492c59f94c6a960d49919c12cb537c4002f0b6d13273a2ccb83af86bf0c0`)
  verdict **PASS_WITH_WARNINGS** — 2 MEDIUM introduced findings, both
  remediation=`backlog`:
  - `coupling-001` — repeated identical dispatch guard block at 10 tool
    handlers (duplicated-func).
  - `overeng-001` — toolset constants overlap (`ALL_TOOL_NAMES` /
    `NATIVE_TOOL_NAMES` / per-language lists).
  Zero CRITICAL/HIGH, no pre-existing findings.

### Specs synced into the vault

- `specs/capabilities-discovery/spec.md` — **NEW** main spec for the
  capability-aware progressive discovery capability
  (`sha256:67ddcc3e613271533269fbfbda9c8f95a231090035f76c636fcdecb03ae12f1d`).
- `specs/capabilities/spec.md` — **NEW** main spec for the modified
  capability `capabilities` (delta merged as initial main: ADDED
  `REQ-CAP-ADD-001` + `REQ-CAP-ADD-002`, MODIFIED `REQ-CAP-MOD-001`).
- 6 new REQ files + 1 modified REQ:
  - `specs/capabilities-discovery/REQ-CapabilitySnapshotToolAvailability.md`
  - `specs/capabilities-discovery/REQ-ChronosActiveToolsetEnvVar.md`
  - `specs/capabilities-discovery/REQ-ListToolsOverrideFilter.md`
  - `specs/capabilities-discovery/REQ-SnapshotAgeViaProbedAt.md`
  - `specs/capabilities-discovery/REQ-ToolAvailabilityKeyedByListTools.md`
  - `specs/capabilities-discovery/REQ-ToolAvailabilityShape.md`
  - `specs/AGENT_API_V2/REQ-CapabilitiesToolAdditiveShape.md` (MODIFIED)

### Vault validation at archive time

`sddk vault validate` returned exit 0, 120 nodes, 62 backlinks, 56 pre-existing
errors (26 VAULT002 duplicate IDs from m0/m2/m8; 30 VAULT003 forward-link gaps
from those same cycles). This cycle resolved 7 VAULT003 forward-link gaps by
creating stubs for `cycles/m10-ms-cap-discovery/proposal.md`,
`changes/archive/m10-ms-cap-discovery/archive-manifest.md`,
`cycles/m10-product-evolution-propose/proposal.md`, and
`adrs/0002-capability-aware-discovery.md`. No new issues introduced.

## Operator-facing behaviour change

The default server behaviour for `tools/list` and `capabilities.call` is
**byte-identical** to before this cycle. The new contract only kicks in when:

- `CHRONOS_ACTIVE_TOOLSET` env var is set to one of `auto|native|js|python|java|go|browser`.
  - `auto` (default) ⇒ behaves like `native`, but will accept override if
    set in production.
  - `native` ⇒ exposes only the language-agnostic core toolset (≤25 native
    tools, REQ-CAP-005).
  - `js|python|java|go|browser` ⇒ narrow the list to the tools that language's
    sandbox actually implements.
- `capabilities` is called and the response includes a non-empty
  `tool_availability: { <tool_name>: { language, status, reason } }` plus
  `probed_at: u64` (epoch seconds of the last internal probe).

The agent API v2 spec was updated to reflect the additive shape.

## Carry-forward (backlog)

These are not blockers, but should be considered next:

1. **`coupling-001`** — refactor the duplicated `is_tool_listed()` / "ok vs
   error" guard that lives at 10 tool handlers. Either lift it to a single
   dispatcher macro, or merge the three toolset constants into one `Lazy`
   table keyed by name → `(toolset_id, language)`.
2. **`overeng-001`** — consolidate `ALL_TOOL_NAMES`,
   `NATIVE_TOOL_NAMES`, and the language toolset constants into a single
   source of truth (probably derived from `list_tools` output at start-up,
   per REQ-CAP-002; today the registry is hand-maintained and `verify`
   flagged the resulting count discrepancy).
3. **REQ-CAP-004 wording**: spec text says "60 tools" but the registry ships
   61. Not a behaviour bug, just outdated prose — update the spec wording or
   add an `ADR-0002-amendment-001` once the count stabilises.
4. **T4-smoke UAT** for `capabilities_uat.rs` was out of A-min scope; worth
   scheduling as an A-lite follow-up to exercise the new filtering in the
   real sandbox (T5 only).

## Pointer for the next session

Two paths remain plausible for m10 follow-up; pick one when resuming:

- **MS-CAP-DISCOVERY-FOLLOWUP** (A-min or A-lite): backlog items 1 and 2 are
  the natural next cycle — they are debt-verified MEDIUM findings introduced
  by this cycle, scoped to one crate each, and close the
  `capabilities-overeng` story opened by ADR-0002.
- **m10-spec-coverage-glue** (A-full): if the broader m10 m-series is
  nearing closure and we want a sync-of-syncs cycle, this would reconcile
  the new `capabilities-discovery` spec with the `AGENT_API_V2/REQs` index,
  then chase any leftover m10 milestones (`MS-PROPERTY-POLICY` is
  closed, `MS-EVT-TYPED` is closed, only this `MS-CAP-DISCOVERY` family
  remains).

Until the user picks, leave the workspace on `main` at `3f7abc35`,
cycle `p-3416cfb8288f8964/m10-ms-cap-discovery` left CLOSED, and the
`feat/m10-ms-cap-discovery` branch intact (archive rule says do not delete
branch before the manifest is persisted, and persistence is done).
