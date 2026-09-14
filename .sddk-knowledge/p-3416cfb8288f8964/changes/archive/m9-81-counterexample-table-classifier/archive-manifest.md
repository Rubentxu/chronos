# Archive Manifest — m9-81-counterexample-table-classifier

## Cycle

| Cycle | `m9-81-counterexample-table-classifier` |
| Date | 2026-09-14 |

## Summary

m9-81 closes FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION by
routing 6 production read-path sites in
`crates/chronos-store/src/counterexample_storage.rs` through the canonical
`chronos_store::table_error::classify_read_table_error()` helper that
m9-72 introduced for `cas.rs` and `storage.rs`. Behaviour-preserving
refactor: 74 / 0 lib unit test counts match before and after.

Path: B-direct. Tiers run: T0 + T1. Wall time: ~30 min.

> **Cycle**: `p-3416cfb8288f8964/m9-81-counterexample-table-classifier`
> **Tag**: `v0.7.83`
> **Merge SHA**: `fdc5accf64be1fcf780913243aec0496ad48e7fe`
> **Date archived**: 2026-09-14
> **Status**: CLOSED

## Summary

m9-81 closes FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION by
routing 6 production read-path sites in
`crates/chronos-store/src/counterexample_storage.rs` through the canonical
`chronos_store::table_error::classify_read_table_error()` helper that
m9-72 introduced for `cas.rs` and `storage.rs`. Behaviour-preserving
refactor: 74 / 0 lib unit test counts match before and after.

Path: B-direct. Tiers run: T0 + T1. Wall time: ~30 min.

## Tag and merge

| Field | Value |
|---|---|
| Base SHA | `45b53df132186b09de75b543b87cf0bab23bd26e` |
| Tag | `v0.7.83` (annotated) |
| Tag peel (commit) | `fdc5accf64be1fcf780913243aec0496ad48e7fe` |
| Head SHA | `fdc5accf64be1fcf780913243aec0496ad48e7fe` |
| Merge commit (--no-ff) | `fdc5accf64be1fcf780913243aec0496ad48e7fe` |
| Peel match | clean |
| Merge strategy | `--no-ff` |
| Origin state | main `a4c4dd97216cdfd2434292c18a0d61a43fcd8166`, tag live on origin |

## Source delta

| File | Change | Lines |
|---|---|---|
| `crates/chronos-store/src/counterexample_storage.rs` | modified | +11 / -13 (net -2) |

## Cycle commits

| SHA | Title |
|---|---|
| `80cca0d` | m9-81: vault (exploration-report + proposal + spec + tasks) |
| `a3f59ea` | m9-81: route 6 counterexample_storage read paths through table_error |
| `93f7cf5` | m9-81: verify-phase cycle-artifacts (apply-checkpoint + receipts) |
| `fdc5acc` | Merge branch 'feat/m9-81-counterexample-table-classifier' into main |
| `a4c4dd9` | m9-81: release-phase artifacts (v0.7.83, merge fdc5acc) |
| `3106fc5` | m9-81: record origin-push state in release-receipt |

## Carry-forward

- **Closed**: FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION.
- **New (out of scope, follow-up cycle)**: FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK
  (the `sddk cycle evaluate-gate` CLI is blocked by a pre-existing
  FOREIGN KEY constraint bug in the ledger DB; m9-81+ cycles will
  need to use the manual vault-tracked workflow until that is fixed).

## Behavioural baseline

- `cargo test -p chronos-store --lib --no-fail-fast`: 74 / 0 (cycle base)
  → 74 / 0 (cycle head) — round-trip verified via `git stash`.
- `cargo test -p chronos-services --lib --no-fail-fast`: 264 / 0
  (downstream smoke; only consumer is `chronos_services::counterexample`).
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --all -- --check`: 0 diffs.

## Findings deferred

None — m9-81 closed its target FIND without introducing any new ones.

## Next roadmap candidate

The m9-roadmap continues. m9+ carry-forwards still unassigned include:

- FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE (m9-75) —
  degraded in-memory mode not surfaced in tool responses.
- FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK (new) — the sddk CLI gate
  evaluation is blocked for this project; needs investigation and
  ledger repair before m9-82+ can use the CLI workflow.

The M7 milestone (deferred from M6 — see `docs/ROADMAP.md`) is
larger-scope work: events_read merge, observe merge,
session_compare+session_explain split, session_start/stop lifecycle,
deprecation sunset sweep.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-81-counterexample-table-classifier/archive-manifest.md` | `c0c2946c1b2fb599676442e6b5158e70b329aff5e6188c0474ec701a9b183602` |
| implementation-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/implementation-receipt.md` | `dba3e0dea8c306c8f3884773730961f5225e3a393d94e8b647ea83fb78e3b3d4` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/verify-report.md` | `ea6bd50b9fa88a222daf314c5d13f8e06e691faf6cf4c27c680385ee339d86f5` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/verify-findings.json` | `9b3883717ea937d237db478d727c667f7033405a9d299f741b6eab0c389e7087` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/release-receipt.md` | `5ecd9f74127e76e61f7d1c7d784e874a6bb11b0e733c9893a161651705bb8d46` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/merge-receipt.md` | `cc2c3b1abf3988b1dd5d2668d1edfd6e87d64bf85108e5df05a62914be75a4be` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/release-report.md` | `c91c2051adbab46a1772030293a0f1cb34e238b0b94c5077af2bb9b6713d78d7` |
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/apply-checkpoint.json` | `6330806dbc2f50f36f344c010ef04e7309d8b3d6d59a3522625e3e6b4a05d992` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/change-entry.md` | `38affdb4edb412842e4f52ebb9fccd53d094d356b8cd2db07394b841e9e7a3ab` |
| exploration-report | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/exploration-report.md` | `fd081a4691b0d51bb0d400bb31c0de94888dc03864d470256f68bd01487bc612` |
| proposal | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/proposal.md` | `95ca700b010ae40a0bff2e37f665195979788f7be38dcd9ae24e41f9a500391a` |
| spec | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/spec.md` | `93e79efac2bb8f0dd76e1668f2e045e3b86eb71c5565dc1cfa9a38db65596b2e` |
| tasks | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/tasks.md` | `379d708e3f0268b46343ac0dc825ca3c9a7b32b4e90b782efc9960a4556b3748` |
| vault index (cycles) | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | `d3802387ec0d18aad0aa0af58c09ae90920ca2884c42bb73dc32e05fdd82c8e5` |
| vault index (terms) | `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | `6fe2f10c6aa46283c8591a5d8e4b52255a5feffcaf13a2d4867cc80ffe559263` |
| source (chronos-store) | `crates/chronos-store/src/counterexample_storage.rs` | (+11/-13: 6 sites refactored; 1 import swap) |

## Evidence bindings

- **`apply-checkpoint.json`**: status CLOSED; verify_status passed; release_status released; archive_status archived (after this commit); `tag = "v0.7.83"`, `tag_peel_sha = main_sha = merge_commit_sha = fdc5accf64be1fcf780913243aec0496ad48e7fe`.
- **`implementation-receipt.md`**: `head_sha = a3f59ea0d1bd446cd20c12f3f62863d70deb0f3a`; `base_sha = 45b53df132186b09de75b543b87cf0bab23bd26e`; 6 sites refactored; all REQs PASS.
- **`verify-report.md`**: PASS verdict; CC#55 inventory present; CC#30A-D title/verdict/cycle/summary all match.
- **`verify-findings.json`**: 5 CLOSED (F-M9-81-01..05), 1 OPEN out-of-scope (F-M9-81-06: sddk CLI gate FK bug).
- **`release-receipt.md`**: clean peel match (`v0.7.83^{commit} == merge fdc5acc`).
- **`merge-receipt.md`**: --no-ff merge; no conflicts; topology preserved.
- **`release-report.md`**: cycle summary, behavioural baseline, carry-forward (FIND-M9-72 closed).
- **`change-entry.md`**: "What changed" + "Why" + "Carry-forward" sections all present.
- **`exploration-report.md`**: F1-F6 recon findings (6 sites, helper shape, test plan, risk profile).
- **`proposal.md`** / **`spec.md`** / **`tasks.md`**: standard SDDK phase artifacts.
- **CC#12**: `main_sha == head_sha == remote_tag_peel == fdc5accf64be1fcf780913243aec0496ad48e7fe`.
- **CC#21** (`## Evidence bindings` in archive-manifest): this section.
- **CC#24 / CC#31** (`## Cross-checks`): present in `change-entry.md`,
  `verify-report.md`, and this manifest.

### File hashes (SHA-256)

| File | SHA-256 |
|---|---|
| `cycle-artifacts/.../apply-checkpoint.json` | `8d221389389e553510a202aaaa48695a1003a575d92f45fd54693ca7e5249aaa` |
| `cycle-artifacts/.../implementation-receipt.md` | `dba3e0dea8c306c8f3884773730961f5225e3a393d94e8b647ea83fb78e3b3d4` |
| `cycle-artifacts/.../verify-report.md` | `a82bfa57d7b710a44333430e3e53c89a2c838603dd8074ca571050b8a3d020e0` |
| `cycle-artifacts/.../verify-findings.json` | `678aeb7b4582075170ae3d6c7cdf6a682458dab103ad164e21881f9011370f4f` |
| `cycle-artifacts/.../release-receipt.md` | `e4421757613485685005cdc3934e08029ec01e02aa4e9a683eb988b104360a36` |
| `cycle-artifacts/.../merge-receipt.md` | `01d604c1eee6e4f7972b312b41e95f45afbeb8c0403412299142c041c1f1025c` |
| `cycle-artifacts/.../release-report.md` | `91fc09b366dd610e0bd637c67b8d743cf1d64daa9a9a1db2da6f57cf6804ac66` |
| `.sddk-knowledge/.../m9-81-.../change-entry.md` | `9960ba6fb831a45401c8953b675059ddc6a5e152d853a4a6b6e6c39a5cda76fe` |
| `.sddk-knowledge/.../m9-81-.../exploration-report.md` | `fd081a4691b0d51bb0d400bb31c0de94888dc03864d470256f68bd01487bc612` |
| `.sddk-knowledge/.../m9-81-.../proposal.md` | `95ca700b010ae40a0bff2e37f665195979788f7be38dcd9ae24e41f9a500391a` |
| `.sddk-knowledge/.../m9-81-.../spec.md` | `93e79efac2bb8f0dd76e1668f2e045e3b86eb71c5565dc1cfa9a38db65596b2e` |
| `.sddk-knowledge/.../m9-81-.../tasks.md` | `379d708e3f0268b46343ac0dc825ca3c9a7b32b4e90b782efc9960a4556b3748` |
| `.sddk-knowledge/.../cycles/index.md` | (regen-tracked by CC#4; current: `6a30b0ea...`) |
| `.sddk-knowledge/.../terms/index.md` | (regen-tracked by CC#4; current: `59a6b701...`) |

## Cross-checks

- **CC#12** (tag peel matches merge SHA): PASS — `v0.7.83^{commit} == fdc5accf64be1fcf780913243aec0496ad48e7fe == HEAD (pre release-artifacts) == main_sha`.
- **CC#28** (base_sha / head_sha full 40-char hex): PASS — base `45b53df132186b09de75b543b87cf0bab23bd26e`, head `a3f59ea0d1bd446cd20c12f3f62863d70deb0f3a`.
- **CC#30A-D** (title/verdict/cycle/summary): PASS — see verify-report.md.
- **CC#32** (Path field present and matches `B-direct`): PASS — apply-checkpoint.json `path: "B-direct"`, verify-report.md path line, archive-manifest.md Path line.
- **CC#36** (lens_summary): PASS — verify-report.md has Behaviour/Code-quality/Architectural lens summaries.
- **CC#42** (peel format): PASS — `v0.7.83^{commit}` is full 40-char hex; `tag_peel_sha = merge_commit_sha`.
- **CC#51** (cycle-artifacts folder): PASS — `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/` contains 7 artifacts (apply-checkpoint.json, implementation-receipt.md, verify-findings.json, verify-report.md, release-receipt.md, merge-receipt.md, release-report.md).
- **CC#55** (Files Inventory): PASS — see "Files Inventory" section above.
- **CC#4** (archive-manifest SHA propagation): PASS — `scripts/regen_manifest_index_shas.py` reports `nothing to do (79 manifest(s) already correct)` after archive phase.


