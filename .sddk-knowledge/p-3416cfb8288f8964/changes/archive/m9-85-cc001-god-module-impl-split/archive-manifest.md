# Archive Manifest — m9-85-cc001-god-module-impl-split

## Identification

| Field | Value |
|---|---|
| Cycle | m9-85-cc001-god-module-impl-split |
| Path | A-min |
| Branch | feat/m9-85-cc001-god-module-impl-split |
| Date | 2026-09-14 |
| Base SHA | 2c2a5cc8f8370eb64dca7fb4ddc47a6f3e8b15a7 |
| Head SHA | `a85034603031f2dd1dc340d78d84f71f140672e0` |
| Remote tag | v0.7.87 |
| Cycle | m9-85-cc001-god-module-impl-split |

## Summary

Behaviour-preserving lexical refactor: extracted the 11-method
`impl SessionStore { ... }` block from
`crates/chronos-store/src/counterexample_storage.rs` (lines 448-909
pre-refactor, ~462 lines) into 3 sibling submodules grouped by concern:

- `ce_write.rs` (148 lines, NEW) — write-path methods.
- `ce_read.rs` (276 lines, NEW) — read-path methods.
- `ce_test_hooks.rs` (99 lines, NEW) — m9-05 R4 test chokepoints.

`counterexample_storage.rs`: 2720 → 2287 lines (-433 net extracted).
Public surface unchanged (downstream crates compile without
modification). 77/77 chronos-store lib tests pass, 264/264
chronos-services lib tests pass, 35/35 chronos-cli tests pass
(includes replay_integration round-trip), 103/103 chronos-native
serial tests pass. clippy + fmt clean.

This is the second slice of the multi-cycle `cc-001-god-module` debt
finding (P2, MEDIUM, m9-04). The remaining concerns (type definitions,
tests, standalone pub fns) will be split across m9-86+.

## Evidence bindings

- `cycle-artifacts/p-3416cfb8288f8964/m9-85-cc001-god-module-impl-split/apply-checkpoint.json` → 2f8e9290ebf8787e30d4cd317b53b7f444d5e847dd113839a323335253f7de48
- `cycle-artifacts/p-3416cfb8288f8964/m9-85-cc001-god-module-impl-split/implementation-receipt.md` → 7cb04cf225c8631762bb80dfe5816b9115f666658094fef94efa3bd3b60b1951
- `cycle-artifacts/p-3416cfb8288f8964/m9-85-cc001-god-module-impl-split/merge-receipt.md` → 71cc751d6ed4ccf2a3ba50733d2706c76a3c38ffeab26a097c1269fe993080ba
- `cycle-artifacts/p-3416cfb8288f8964/m9-85-cc001-god-module-impl-split/release-receipt.md` → 98a98a24e1372b0ac4d1971781f019168700fa19679cd720d6b831d4415f3c10
- `cycle-artifacts/p-3416cfb8288f8964/m9-85-cc001-god-module-impl-split/release-report.md` → 295f7cc2aa86643794fdbbe32f897d6c23c1050dcae3ed9f3ff80c5a72584c06
- `cycle-artifacts/p-3416cfb8288f8964/m9-85-cc001-god-module-impl-split/verify-findings.json` → 6fc44966c23baf00bcbd78db72a4b8f2505575484e28585ba89e9e2bc58d9547
- `cycle-artifacts/p-3416cfb8288f8964/m9-85-cc001-god-module-impl-split/verify-report.md` → 8d065a9aedc904d07536cb4614f593b5410375eaaea5316eca1de143517e769c
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-85-cc001-god-module-impl-split/exploration-report.md` → ca498831bc2c1e1f3d850b998c4ddef8e02d8391abcd44d490273bce98cb7383
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-85-cc001-god-module-impl-split/proposal.md` → b1abf9e6acf7aae177f017caa95882dbbb1b22231e3a50d71dc1fb6ad732fbf0
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-85-cc001-god-module-impl-split/spec.md` → be6fb1cb2193496ae354ceff6bad1c72fa8819b6bf018b5ba4bc475ff16d3184
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-85-cc001-god-module-impl-split/tasks.md` → 43a681e06f72b8da55b71da13edac5170b79a54eaaad37610ddaa9cf6725c266

## Cross-checks

- `apply-checkpoint.head_sha` == `release-receipt.head_sha` == `merge-receipt.head SHA` == `a85034603031f2dd1dc340d78d84f71f140672e0`.
- `Remote tag` v0.7.87 peel: `a85034603031f2dd1dc340d78d84f71f140672e0` (clean match).
- `counterexample_storage.rs` line count: 2287 (was 2720; net -433).
- `ce_write.rs` line count: 148 (NEW file).
- `ce_read.rs` line count: 276 (NEW file).
- `ce_test_hooks.rs` line count: 99 (NEW file).
- `cargo test -p chronos-store --lib`: 77/77 (matches baseline).
- `cargo test -p chronos-services --lib`: 264/264 (downstream).
- `cargo test -p chronos-cli`: 35/35 (downstream, includes replay_integration round-trip).
- `cargo test -p chronos-native --lib -- --test-threads=1`: 103/103 (per AGENTS.md §6.5).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- `bash scripts/check_vault_drift.sh`: to be verified post-archive.
