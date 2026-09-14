# Archive Manifest — m9-86-cc001-god-module-types-split

## Identification

| Field | Value |
|---|---|
| Cycle | m9-86-cc001-god-module-types-split |
| Path | A-min |
| Branch | feat/m9-86-cc001-god-module-types-split |
| Date | 2026-09-14 |
| Base SHA | 43e48b927eebb30be5cd2c6fcffbd6dcbabcbc7d |
| Head SHA | 434f2b4db74e94488f180100a8e8c8db50fd7fa3 |
| Remote tag | v0.7.88 |
| Cycle | m9-86-cc001-god-module-types-split |

## Summary

Behaviour-preserving lexical refactor: extracted the 6 `pub` types +
the `default_schema_version` serde helper from
`crates/chronos-store/src/counterexample_storage.rs` (lines 327-513
post-m9-85, ~187 lines) into a new sibling submodule
`crates/chronos-store/src/ce_types.rs` (165 lines, NEW).

`counterexample_storage.rs`: 2287 → 2156 lines (-131 net extracted).
Public surface unchanged (all callers continue to use
`chronos_store::counterexample_storage::TypeName` paths, no source
modification). 77/77 chronos-store lib tests pass, 264/264
chronos-services lib tests pass, 35/35 chronos-cli tests pass
(includes replay_integration round-trip), 103/103 chronos-native
serial tests pass. clippy + fmt clean.

This is the third slice of the multi-cycle `cc-001-god-module` debt
finding (P2, MEDIUM, m9-04). The remaining concerns (free functions
`collect_*`, `KNOWN_BUNDLE_SCHEMA_VERSIONS`; tests ~1,750 lines) will
be split across m9-87+.

## Evidence bindings

- `cycle-artifacts/p-3416cfb8288f8964/m9-86-cc001-god-module-types-split/apply-checkpoint.json` → fe0422c3dc9110fdc0b5921fc2b6197f5057c8614b111ad58f52380115e2130f
- `cycle-artifacts/p-3416cfb8288f8964/m9-86-cc001-god-module-types-split/implementation-receipt.md` → 4a5099206c58a77867caf9d67571325b19608d411ae5e9a76d596b2e91c480f1
- `cycle-artifacts/p-3416cfb8288f8964/m9-86-cc001-god-module-types-split/merge-receipt.md` → 9ba8df55a3cd7d4ad6bfeecf802e7d0b341e03c9c85007897eee3d61ca7c76fb
- `cycle-artifacts/p-3416cfb8288f8964/m9-86-cc001-god-module-types-split/release-receipt.md` → fa9afd51006ff7d9ecb0645b152595e6122d1be4119c4ea8511306e07c4f43db
- `cycle-artifacts/p-3416cfb8288f8964/m9-86-cc001-god-module-types-split/release-report.md` → b9933dea9fa5e327c8742ddf33ec28981d50919a1e6effd07492ee8363ac4989
- `cycle-artifacts/p-3416cfb8288f8964/m9-86-cc001-god-module-types-split/verify-findings.json` → a08a2174ef14966cdbbb292ec9998a13a07119e05d8574976ac2e0dfdf1a6346
- `cycle-artifacts/p-3416cfb8288f8964/m9-86-cc001-god-module-types-split/verify-report.md` → c62dfaa182feee855198a97d7fa097d461ca225258613b4676a6504abeca88c8
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-86-cc001-god-module-types-split/exploration-report.md` → 55da103bc0a82da0f8e00f66f517c1c62857b59e743f1cd40983528b992801d2
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-86-cc001-god-module-types-split/proposal.md` → 7eb2ddf70bfb70c1f4fd08d1f82ee8ed00e6b3593f97cb285804c5912606de80
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-86-cc001-god-module-types-split/spec.md` → 7f726f84e9c8d9b82b2c2615e59397954f6e95ddcfe5ecfe47c24090f4445307
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-86-cc001-god-module-types-split/tasks.md` → 1cd18adb4aaf407fcd8a77c0de71d055485df90c81af0c956e319c0d647f9938

## Cross-checks

- `apply-checkpoint.head_sha` == `release-receipt.head_sha` == `merge-receipt.head SHA` == `434f2b4db74e94488f180100a8e8c8db50fd7fa3`.
- `Remote tag` v0.7.88 peel: `434f2b4db74e94488f180100a8e8c8db50fd7fa3` (clean match).
- `counterexample_storage.rs` line count: 2156 (was 2287; net -131).
- `ce_types.rs` line count: 165 (NEW file).
- `cargo test -p chronos-store --lib`: 77/77 (matches baseline).
- `cargo test -p chronos-services --lib`: 264/264 (downstream).
- `cargo test -p chronos-cli`: 35/35 (downstream, includes replay_integration round-trip).
- `cargo test -p chronos-native --lib -- --test-threads=1`: 103/103 (per AGENTS.md §6.5).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- `bash scripts/check_vault_drift.sh`: to be verified post-archive.
