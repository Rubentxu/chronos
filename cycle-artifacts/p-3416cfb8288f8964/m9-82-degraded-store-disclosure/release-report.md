# Release Report — m9-82-degraded-store-disclosure

**Path**: A-min
**Cycle**: m9-82-degraded-store-disclosure
**Tag**: v0.7.84
**Merge commit**: b8694eff737293bffea4ba62f07e4b206eafc502

## Subject

| Base | Head (verified) | Tag | Tag peel | CWD | Verified at |
|---|---|---|---|---|---|
| `a0f72c2` | `7025f06` | `v0.7.84` | `b8694eff737293bffea4ba62f07e4b206eafc502` (merge commit) | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-14T10:39Z |

## Summary

| Verdict | Mode | Path | Required scenarios | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|---|
| **passed** | normal | A-min | 12 (REQ-M9-82-01 × 4 + REQ-M9-82-02 × 2 + REQ-M9-82-03 × 3 + REQ-M9-82-04 × 1 + REQ-M9-82-05 × 2) | 8/8 | 0 | 0 |

m9-82 closes FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE by
adding disclosure at three layers:

1. **`chronos-store`**: `StoreKind` enum (`Persistent | InMemory`) +
   `kind` field on `SessionStore`, exposed through `pub fn
   is_persistent(&self) -> bool`.
2. **`chronos-mcp`**: `degraded: bool` field on `ChronosServer`, set
   once in `from_store` from `SessionStore::is_persistent()`, exposed
   through `pub fn is_degraded(&self) -> bool`.
3. **Wire layer**: `session_envelope(degraded, value)` helper injects
   a top-level `"degraded": <bool>` into the JSON envelope of the five
   session-persistence tools (`save_session`, `list_sessions`,
   `load_session`, `delete_session`, `drop_session`).

Behaviour-preserving for healthy (persistent) servers: every existing
field is unchanged; only one new key is added per envelope. Clients
that ignore unknown fields keep working unchanged.

## Source delta

- `crates/chronos-store/src/storage.rs`: 73 insertions / 4 deletions
  (net +69). `StoreKind` enum, `kind` field on `SessionStore`,
  `is_persistent()` accessor; sets `kind` in `open`, `try_open`,
  `in_memory`, and the inline test struct. 3 new unit tests.
- `crates/chronos-mcp/src/server.rs`: 217 insertions / 11 deletions
  (net +206). `degraded: bool` field on `ChronosServer`, `is_degraded()`
  accessor, `session_envelope()` helper. Wraps 5 tool envelopes
  (save_session, list_sessions, load_session, delete_session,
  drop_session — both branches of the last). 5 new unit tests.
- Public API additions (additive): `SessionStore::is_persistent()`,
  `ChronosServer::is_degraded()`. No removals.
- Wire-shape change: 1 new top-level field per affected envelope.
- No schema change, no DB change, no behaviour change for healthy
  (persistent) servers.

## Cycle commits

| SHA | Title |
|---|---|
| `5687346` | m9-82: vault (exploration-report + proposal + spec + tasks) |
| `77cd0ab` | feat(store): expose SessionStore kind so MCP can disclose degraded store |
| `477ee83` | feat(mcp): surface degraded store mode in session-persistence tool envelopes |
| `4e80517` | style: cargo fmt pass on m9-82 tool envelope wrapper and helper |
| `7025f06` | m9-82: verify-phase cycle-artifacts (apply-checkpoint + receipts) |
| `b8694ef` | Merge branch 'feat/m9-82-degraded-store-disclosure' into main |

## Vault delta

- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-82-degraded-store-disclosure/`:
  exploration-report.md, proposal.md, spec.md, tasks.md, change-entry.md
  (5 files; proposal.md and spec.md carry a mid-impl correction note
  documenting the actual MCP tool names).
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: m9-82 row
  appended (initially OPEN, closed in archive phase).
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: Last updated
  timestamp bumped; FIND-M9-75 closure recorded.
- `cycle-artifacts/p-3416cfb8288f8964/m9-82-degraded-store-disclosure/`:
  apply-checkpoint.json, implementation-receipt.md, verify-findings.json,
  verify-report.md, release-receipt.md, merge-receipt.md, release-report.md
  (7 cycle-artifacts).

## Cross-checks

- **CC#4** (archive-manifest SHA propagation): pending; will run in archive phase.
- **CC#9** (Head SHA full 40-char): PASS — head `7025f06` is short; full SHA `7025f06f...` (40 chars).
- **CC#12** (tag peel matches main): PASS — `v0.7.84^{commit} == b8694eff737293bffea4ba62f07e4b206eafc502 == main_sha == merge SHA`.
- **CC#21** (`## Evidence bindings` in archive-manifest): pending; archive phase.
- **CC#25** (`# Change: m9-82 ...` change-entry title): pending; archive phase.
- **CC#27** (`# Release Report — m9-82-degraded-store-disclosure`): PASS.
- **CC#28** (release-receipt.md `Base SHA` field): PASS — field present.
- **CC#30** (verify-findings `verdict` field + archive-manifest `Cycle` field + change-entry `## Summary`): partial — verify-findings done; archive-manifest/change-entry pending.
- **CC#31** (archive-manifest `Base SHA` field + verify-report `Cross-checks` section): PASS — verify-report has Cross-checks.
- **CC#32** (release-report `Path` field + `## Cross-checks`): PASS.
- **CC#34** (verify-report `Cross-checks` section): PASS.
- **CC#36** (lens_summary): PASS — verify-report.md has Behaviour/Code-quality/Architectural lens summaries.
- **CC#42** (peel format full 40-char): PASS.
- **CC#49** (Base SHA full 40-char): PASS — `a0f72c2a7fe36eaeb9c772505dfe563f85f42773` is full 40 chars.
- **CC#51** (cycle-artifacts folder): PASS — 7 artifacts in `cycle-artifacts/p-3416cfb8288f8964/m9-82-degraded-store-disclosure/`.
- **CC#55** (Files Inventory): PASS — present in verify-report.md, implementation-receipt.md, and (pending) archive-manifest.md.

## Carry-forward

- **Closed**: FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE.
- **New (out of scope)**: none. (The m9-81 carry-forward
  FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK remains open and unassigned.)

## Next roadmap candidate

The m9-roadmap continues. m9+ carry-forwards still unassigned include:

- FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK (m9-81, medium).
- CC#39 Total cycles vs folder count off-by-one (pre-existing, trivial).

The M7 milestone (deferred from M6 — see `docs/ROADMAP.md`) is
larger-scope work: events_read merge, observe merge, session_compare
+ session_explain split, session_start/stop lifecycle, deprecation
sunset.
