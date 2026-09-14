# Archive Manifest — m9-94-cc001-test-split

## Identification

| Field | Value |
|---|---|
| Cycle | m9-94-cc001-test-split |
| Path | B-direct (mechanical refactor, no behavior change) |
| Branch | chore/m9-94-cc001-test-split |
| Date | 2026-09-14 |
| Base SHA | 645eedb530f1b827a600544d01e501d82b410820 |
| Head SHA | `9e15dd3fbfc5319df8bd31ba540d570c8fd75ad8` |
| Merge SHA | 380a452fd0dc6d7e5e85f99dec97efbe47cf607c |
| Source SHA | 3a8494967c366761f10da6caf89745971fce9f78 |
| Cascade SHA | dea23db7d19946346cff3346b2bf03826fb8abf2 |
| Index-cascade SHA | 8c25fe3a5307dcdd1fc209e00a0d14bae0a72a82 |
| Remote tag | v0.7.96 |
| Tag peel SHA | 9e15dd3fbfc5319df8bd31ba540d570c8fd75ad8 |

## Summary

B-direct mechanical refactor cycle. Closes
FIND-M9-94-CC001-TEST-CODE-MONOLITHIC by extracting the 1771-line inline
`mod tests { ... }` block from
`crates/chronos-store/src/counterexample_storage.rs` (lines 188-1960)
into a sibling file `crates/chronos-store/src/ce_storage_tests.rs`
using the `#[path = "..."]` submodule pattern established by
m9-84..m9-87.

After m9-94: `counterexample_storage.rs` shrinks from 1960 → 191 lines
(production code only); `ce_storage_tests.rs` holds the 43 test
functions + 4 test helpers in a sibling submodule reachable as
`counterexample_storage::tests`. The Rust module graph is identical
pre- and post-extraction (same items, same visibility, same
resolution paths).

## Drift delta

None. m9-94 is a Rust-only cycle; no vault CC drift introduced or
closed.

## Files changed

| Bucket | Files | Lines | Notes |
|---|---|---|---|
| crates/chronos-store/src/ce_storage_tests.rs | 1 added | +1785 | NEW sibling file (verbatim copy of inline test block + 14-line doc comment header) |
| crates/chronos-store/src/counterexample_storage.rs | 1 modified | -1772 / +3 | replaced 1773-line inline `mod tests { ... }` block with 3-line `#[path = "..."]` declaration |
| m9-94 cycle artifacts | 7 added | — | apply-checkpoint + 6 receipts/reports |
| m9-94 knowledge artifacts | 5 added | — | proposal + spec + tasks + exploration + change-entry |
| cycles/index.md | 1 modified | +2 | m9-94 row + Total 93 → 94 |
| terms/index.md | 1 modified | +1 | Last archive = m9-94-cc001-test-split |

**Total**: 2 source files modified + 12 cycle/knowledge artifacts + 2 index files.

## Cross-checks

- `bash scripts/check_vault_drift.sh`: PASS (48 python CCs + 7 bash CCs all clean).
- `python3 scripts/regen_manifest_index_shas.py --check`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p chronos-store --lib --no-fail-fast`: 77 pass.
- `cargo test --workspace --lib --no-fail-fast -- --test-threads=1`: 1042 pass (same as m9-93 baseline).
- `apply-checkpoint.peel_match == true`.
- `apply-checkpoint.head_sha == release-receipt.Head SHA == 9e15dd3`.
- `Remote tag` v0.7.96 peel: `9e15dd3` (cycle-artifacts commit; tag pre-created at cycle-artifacts per CC#42 workaround).
- `apply-checkpoint.status == "CLOSED"`.
- `apply-checkpoint.archive_status == "complete"`.
- `apply-checkpoint.findings_introduced.no_action == []` (cc#19-compliant).
- `cycles/index.md` Total cycles = 94 (matches actual folder count).
- `terms/index.md` Last archive = m9-94-cc001-test-split.
- No new tests added (43 tests + 4 helpers moved verbatim).
- No regressions in 1042 lib tests.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-94-cc001-test-split/apply-checkpoint.json` | `773423836484c7b1361a8a7afc1db26cda1c03346a63c55ddde8ef5aff2ece30` |
| implementation-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-94-cc001-test-split/implementation-receipt.md` | `1c30c4d7b030b0ad095ca8fc0abb47c8d51489793b2779cf0da2e84e4fb2985c` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-94-cc001-test-split/merge-receipt.md` | `594ce7ac282da3c21646d586dee9920396b952c24737a91cbfe8cd78cd368acc` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-94-cc001-test-split/release-receipt.md` | `bfbe952937e015e91f6851f049da2d4972acf00e6e62a2aea2e77c8580785d36` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-94-cc001-test-split/release-report.md` | `df6e795b949c4098a2bdf283188cf85eb174c21d0f3dafc5aa7ad59c72d59947` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-94-cc001-test-split/verify-findings.json` | `6d0dadcb3b072ab59cc96ed03772c7fcffe393c21776cef9d180b32a06f83a19` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-94-cc001-test-split/verify-report.md` | `ae3832778ea1e5316785e9d4ba1648c8d0bbfe73332790d20daa8e2c8aab00ac` |
| proposal | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-94-cc001-test-split/proposal.md` | `e467aa8c781ce6cb5c817662a987d9a90eca8ce70d2b9247066df162b5757592` |
| spec | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-94-cc001-test-split/spec.md` | `31b3e0642cb717a2c91582a9759232961b26ed49ae8e7f29acab80188be177cc` |
| tasks | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-94-cc001-test-split/tasks.md` | `145a4546a48c142685afae9736c191f1202de9e5fe018ece1f6ad19ed9317498` |
| exploration-report | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-94-cc001-test-split/exploration-report.md` | `bb37e08ce121d05692bfca4486c015e8d6662be470009bccadc227eb94d6fe91` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-94-cc001-test-split/change-entry.md` | `666e2b09cf5b853a1f1010b8b23ef55bc81f1153d183ea8f5d9bb25f30c27e59` |

## Evidence bindings

The Artifact index above provides the SHA-256 binding between each
released artifact and the file content at archive time. Readers can
verify each binding with:

```bash
sha256sum <path>  # compare against the SHA-256 listed in the index
```

**Commit provenance:**

| Artifact | Bound to |
|---|---|
| Source commit (Rust) | `3a8494967c366761f10da6caf89745971fce9f78` (m9-94: extract cc-001 storage tests) |
| Cycle artifacts commit | `9e15dd3fbfc5319df8bd31ba540d570c8fd75ad8` (m9-94: cycle artifacts + knowledge files) |
| SHA-cascade commit | `dea23db7d19946346cff3346b2bf03826fb8abf2` (m9-94: align artifacts to cycle-artifacts SHA) |
| Index-cascade commit | `8c25fe3a5307dcdd1fc209e00a0d14bae0a72a82` (m9-94: add cycle row + bump Total to 94) |
| Merge commit | `380a452fd0dc6d7e5e85f99dec97efbe47cf607c` (--no-ff merge into main) |
| Merge-receipt-fill commit | `f5ccae06` (m9-94: fill merge SHA 380a452f in merge-receipt.md) |
| Tag v0.7.96 | `9e15dd3` (pre-created at cycle-artifacts per CC#42 workaround) |
