# Archive Manifest — m9-82-degraded-store-disclosure

## Cycle

| Cycle | `m9-82-degraded-store-disclosure` |

## Summary

m9-82 closes FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE by
adding disclosure at three layers:

1. **`chronos-store`**: `StoreKind` enum (`Persistent | InMemory`) +
   `kind` field on `SessionStore`, exposed through
   `pub fn is_persistent(&self) -> bool`.
2. **`chronos-mcp`**: `degraded: bool` field on `ChronosServer`, set
   once in `from_store` from `SessionStore::is_persistent()`, exposed
   through `pub fn is_degraded(&self) -> bool`.
3. **Wire layer**: `session_envelope(degraded, value)` helper injects
   a top-level `"degraded": <bool>` into the JSON envelope of the five
   session-persistence tools (`save_session`, `list_sessions`,
   `load_session`, `delete_session`, `drop_session`).

Behaviour-preserving for healthy (persistent) servers: every existing
envelope field is preserved unchanged; only one new key is added per
envelope.

Path: A-min. Tiers run: T0 + T2 + focused T3 + T4-smoke. Wall time: ~95 min.

> **Cycle**: `p-3416cfb8288f8964/m9-82-degraded-store-disclosure`
> **Tag**: `v0.7.84`
> **Merge SHA**: `b8694eff737293bffea4ba62f07e4b206eafc502`
> **Date archived**: 2026-09-14
> **Status**: CLOSED

## Summary

m9-82 closes FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE by
adding disclosure at three layers:

1. **`chronos-store`**: `StoreKind` enum (`Persistent | InMemory`) +
   `kind` field on `SessionStore`, exposed through
   `pub fn is_persistent(&self) -> bool`.
2. **`chronos-mcp`**: `degraded: bool` field on `ChronosServer`, set
   once in `from_store` from `SessionStore::is_persistent()`, exposed
   through `pub fn is_degraded(&self) -> bool`.
3. **Wire layer**: `session_envelope(degraded, value)` helper injects
   a top-level `"degraded": <bool>` into the JSON envelope of the five
   session-persistence tools (`save_session`, `list_sessions`,
   `load_session`, `delete_session`, `drop_session`).

Behaviour-preserving for healthy (persistent) servers: every existing
envelope field is preserved unchanged; only one new key is added per
envelope.

Path: A-min. Tiers run: T0 + T2 + focused T3 + T4-smoke. Wall time: ~95 min.

## Tag and merge

| Field | Value |
|---|---|
| Base SHA | `a0f72c2a7fe36eaeb9c772505dfe563f85f42773` |
| Tag | `v0.7.84` (annotated) |
| Tag peel (commit) | `b8694eff737293bffea4ba62f07e4b206eafc502` |
| Merge commit (--no-ff) | `b8694eff737293bffea4ba62f07e4b206eafc502` |
| Peel match | clean |
| Merge strategy | `--no-ff` |
| Origin state | main `e02046e`, tag `v0.7.84` live on origin |

## Source delta

| File | Change | Lines |
|---|---|---|
| `crates/chronos-store/src/storage.rs` | modified | +73 / -4 (net +69) |
| `crates/chronos-mcp/src/server.rs` | modified | +217 / -11 (net +206) |

## Cycle commits

| SHA | Title |
|---|---|
| `5687346` | m9-82: vault (exploration-report + proposal + spec + tasks) |
| `77cd0ab` | feat(store): expose SessionStore kind so MCP can disclose degraded store |
| `477ee83` | feat(mcp): surface degraded store mode in session-persistence tool envelopes |
| `4e80517` | style: cargo fmt pass on m9-82 tool envelope wrapper and helper |
| `7025f06` | m9-82: verify-phase cycle-artifacts (apply-checkpoint + receipts) |
| `b8694ef` | Merge branch 'feat/m9-82-degraded-store-disclosure' into main |
| `87eeec3` | m9-82: release-phase artifacts (v0.7.84, merge b8694ef) |
| `e02046e` | m9-82: record origin-push state in release-receipt |

## Carry-forward

- **Closed**: FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE.
- **New (out of scope)**: none.

## Behavioural baseline

- `cargo test -p chronos-store --lib --no-fail-fast`: 74 / 0 (cycle base `a0f72c2`)
  → 77 / 0 (cycle head) — `+3` new `is_persistent` tests.
- `cargo test -p chronos-mcp --lib --no-fail-fast`: 82 / 0 (cycle base)
  → 87 / 0 (cycle head) — `+5` new degraded-envelope tests.
- `cargo test -p chronos-services --lib --no-fail-fast`: 264 / 0
  (downstream smoke; consumer of `SessionStore` is the `SessionsContext`).
- `cargo test -p chronos-store -p chronos-mcp -p chronos-services --tests --no-fail-fast`:
  477 / 0 / 0 across 10 binaries (focused T3).
- T4-smoke `chronos-sandbox` subset (e2e_connectivity 1 + session_persistence 4 + session_lifecycle 8): 13 / 0 / 0.
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --all -- --check`: 0 diffs.

## Findings deferred

None — m9-82 closed its target FIND without introducing any new ones.

## Next roadmap candidate

The m9-roadmap continues. m9+ carry-forwards still unassigned include:

- FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK (m9-81, medium) — the `sddk cycle
  evaluate-gate` CLI is blocked by a pre-existing FOREIGN KEY constraint
  bug in the ledger DB; m9-81+ cycles continue to use the manual
  vault-tracked workflow until that is fixed.
- FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL (m9-74, informational) —
  the `chronos-native` lib suite must run with `--test-threads=1`
  because of ptrace-test collisions. Documented in AGENTS.md §6.5.
- CC#39 Total cycles vs folder count off-by-one (pre-existing, trivial)
  — cycles/index.md says 84, folder count is 80 (the m9-78 row exists
  but no folder was created). Cosmetic drift only; resolves when the
  vault is reorganized.
- cc-001-god-module (counterexample_storage.rs at 2,821 lines, 5 concerns;
  A-lite split).

The M7 milestone (deferred from M6 — see `docs/ROADMAP.md`) is
larger-scope work: events_read merge, observe merge,
session_compare + session_explain split, session_start/stop lifecycle,
deprecation sunset sweep.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-82-degraded-store-disclosure/archive-manifest.md` | `c0c2946c1b2fb599676442e6b5158e70b329aff5e6188c0474ec701a9b183602` |
| implementation-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-82-degraded-store-disclosure/implementation-receipt.md` | `7a8fbbb7e3531e48cf890652f51ca204e78855803218152da8ff86539f9b80c5` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-82-degraded-store-disclosure/verify-report.md` | `81aea015a12b70afd726fba9012ca3c9cbfc10f9409dd829a1ca9552569e3338` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-82-degraded-store-disclosure/verify-findings.json` | `f6a012ebdf9edba0b64e8acfd6c82a51efb34ce550f9a3d24d68f132e823a902` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-82-degraded-store-disclosure/release-receipt.md` | `449fca22dd27434adf39f978054672a17a7372ebe0fc2dd61c49ba4d5eca7781` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-82-degraded-store-disclosure/merge-receipt.md` | `0ed4d1d74e8c3b41d7568005abb64c0c26f663a7eea3bffa26c529cdab14b85b` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-82-degraded-store-disclosure/release-report.md` | `545099bf6dd0136e10822b10edf1e4a1bd7f046a5eccbbb730d15fc76b66d2e7` |
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-82-degraded-store-disclosure/apply-checkpoint.json` | `3bab50b1385f3947e08b95f490acfffac230f27587b1e0c3e692c7eca8fe2b64` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-82-degraded-store-disclosure/change-entry.md` | `e27f3f9945d5f7672f3b285f8608543363a80b790d4066e918a0b0429c8853c2` |
| exploration-report | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-82-degraded-store-disclosure/exploration-report.md` | `67f941391b7759dac212bcaf2ec1e6d52685f6185633fbf6d5f44934202a0bce` |
| proposal | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-82-degraded-store-disclosure/proposal.md` | `528f46a9318205005ef934c3d9b433fbfab21539574bd312c21f17c75534733b` |
| spec | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-82-degraded-store-disclosure/spec.md` | `a08edb8b2a5bf524dcec12022d59a6558349e2ed3aa78c8253fd4613987a77e1` |
| tasks | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-82-degraded-store-disclosure/tasks.md` | `88a883a34f0f5197c1db7b3d98c338e6a88e02999461cb71de5bdf337db19e56` |
| vault index (cycles) | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | `836784628884de04cb4f7313a3804f34015174f15a205b4ea4e5f6fb1eed01ff` |
| vault index (terms) | `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | `78a10c86210da4f7dc08941ad4e71bec067f0b30e7c92f84248bf70615cd4f93` |
| source (chronos-store) | `crates/chronos-store/src/storage.rs` | (+73/-4: StoreKind enum + kind field + is_persistent() accessor + 3 unit tests) |
| source (chronos-mcp) | `crates/chronos-mcp/src/server.rs` | (+217/-11: degraded field + is_degraded() accessor + session_envelope() helper + 5 tool-envelope wrappers + 5 unit tests) |

## Evidence bindings

- **`apply-checkpoint.json`**: status CLOSED; verify_status passed; release_status released; archive_status archived (after this commit); `tag = "v0.7.84"`, `tag_peel_sha = main_sha = merge_commit_sha = b8694eff737293bffea4ba62f07e4b206eafc502`.
- **`implementation-receipt.md`**: `head_sha = 4e805174fb872e9f4fa0d1ef9d379ba454091994`; `base_sha = a0f72c2a7fe36eaeb9c772505dfe563f85f42773`; 8 unit tests added; all REQs PASS.
- **`verify-report.md`**: PASS verdict; CC#55 inventory present; CC#30A-D title/verdict/cycle/summary all match.
- **`verify-findings.json`**: 7 CLOSED (F-M9-82-01..07), 0 OPEN out-of-scope.
- **`release-receipt.md`**: clean peel match (`v0.7.84^{commit} == merge b8694ef`).
- **`merge-receipt.md`**: --no-ff merge; no conflicts; topology preserved.
- **`release-report.md`**: cycle summary, behavioural baseline, carry-forward (FIND-M9-75 closed).
- **`change-entry.md`**: "Summary" + "Subject" + "Files changed" + "Behavioural impact" + "Cross-check" sections all present.
- **`exploration-report.md`**: recon findings around FIND-M9-75 root cause + the three-layer disclosure design.
- **`proposal.md`** / **`spec.md`** / **`tasks.md`**: standard SDDK phase artifacts (spec and proposal carry a mid-impl correction note).
- **CC#12**: `main_sha == head_sha == remote_tag_peel == b8694eff737293bffea4ba62f07e4b206eafc502`.
- **CC#21** (`## Evidence bindings` in archive-manifest): this section.
- **CC#24 / CC#31** (`## Cross-checks`): present in `change-entry.md`, `verify-report.md`, and this manifest.

### File hashes (SHA-256)

| File | SHA-256 |
|---|---|
| `cycle-artifacts/.../apply-checkpoint.json` | `d1deb28aae73c8d79e13898563d400d5dbc3298461d50369e83cedb02b24f6c5` |
| `cycle-artifacts/.../implementation-receipt.md` | `7a8fbbb7e3531e48cf890652f51ca204e78855803218152da8ff86539f9b80c5` |
| `cycle-artifacts/.../verify-report.md` | `2343e8a662810f23d46aaa22bb34d9e7e7d9e3430624644bd4f1621eee9ed578` |
| `cycle-artifacts/.../verify-findings.json` | `f6a012ebdf9edba0b64e8acfd6c82a51efb34ce550f9a3d24d68f132e823a902` |
| `cycle-artifacts/.../release-receipt.md` | `449fca22dd27434adf39f978054672a17a7372ebe0fc2dd61c49ba4d5eca7781` |
| `cycle-artifacts/.../merge-receipt.md` | `0ed4d1d74e8c3b41d7568005abb64c0c26f663a7eea3bffa26c529cdab14b85b` |
| `cycle-artifacts/.../release-report.md` | `545099bf6dd0136e10822b10edf1e4a1bd7f046a5eccbbb730d15fc76b66d2e7` |
| `.sddk-knowledge/.../m9-82-.../change-entry.md` | `e27f3f9945d5f7672f3b285f8608543363a80b790d4066e918a0b0429c8853c2` |
| `.sddk-knowledge/.../m9-82-.../exploration-report.md` | `67f941391b7759dac212bcaf2ec1e6d52685f6185633fbf6d5f44934202a0bce` |
| `.sddk-knowledge/.../m9-82-.../proposal.md` | `528f46a9318205005ef934c3d9b433fbfab21539574bd312c21f17c75534733b` |
| `.sddk-knowledge/.../m9-82-.../spec.md` | `a08edb8b2a5bf524dcec12022d59a6558349e2ed3aa78c8253fd4613987a77e1` |
| `.sddk-knowledge/.../m9-82-.../tasks.md` | `88a883a34f0f5197c1db7b3d98c338e6a88e02999461cb71de5bdf337db19e56` |
| `.sddk-knowledge/.../cycles/index.md` | (regen-tracked by CC#4; current: `e5c7291b...`) |
| `.sddk-knowledge/.../terms/index.md` | (regen-tracked by CC#4; current: `717ae70f...`) |

## Cross-checks

- **CC#12** (tag peel matches merge SHA): PASS — `v0.7.84^{commit} == b8694eff737293bffea4ba62f07e4b206eafc502 == HEAD (pre release-artifacts) == main_sha`.
- **CC#28** (base_sha / head_sha full 40-char hex): PASS — base `a0f72c2a7fe36eaeb9c772505dfe563f85f42773`, head `4e805174fb872e9f4fa0d1ef9d379ba454091994`.
- **CC#30A-D** (title/verdict/cycle/summary): PASS — see verify-report.md.
- **CC#32** (Path field present and matches `A-min`): PASS — apply-checkpoint.json `path: "A-min"`, verify-report.md path line, archive-manifest.md Path line.
- **CC#36** (lens_summary): PASS — verify-report.md has Behaviour/Code-quality/Architectural lens summaries.
- **CC#42** (peel format): PASS — `v0.7.84^{commit}` is full 40-char hex; `tag_peel_sha = merge_commit_sha`.
- **CC#51** (cycle-artifacts folder): PASS — `cycle-artifacts/p-3416cfb8288f8964/m9-82-degraded-store-disclosure/` contains 7 artifacts (apply-checkpoint.json, implementation-receipt.md, verify-findings.json, verify-report.md, release-receipt.md, merge-receipt.md, release-report.md).
- **CC#55** (Files Inventory): PASS — see "Artifact index" section above.
- **CC#4** (archive-manifest SHA propagation): PASS — `python3 scripts/regen_manifest_index_shas.py` to fixpoint after archive phase; cycles/index.md and terms/index.md SHA rows updated.
