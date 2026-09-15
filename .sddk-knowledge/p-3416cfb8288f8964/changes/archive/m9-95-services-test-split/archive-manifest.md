# Archive Manifest — m9-95-services-test-split

## Identification

| Field | Value |
|---|---|
| Cycle | m9-95-services-test-split |
| Path | B-direct (mechanical refactor, no behavior change) |
| Branch | chore/m9-95-services-test-split |
| Date | 2026-09-14 |
| Base SHA | 72bff2808457a4ead6f4caec233dc404a20c35d8 |
| Head SHA | `8ff34170fe98fd14cc1e10e30e95d842fe67f0f0` |
| Merge SHA | a33b84768026044b4500d677793b5654c410887b |
| Source SHA | eb96861b17711f7d525cedacccc17905bc17e6e0 |
| Cascade SHA | 38fba83c67c3985f85df190fb3c72259d035b8a6 |
| Index-cascade SHA | 3a195111e26e6cd3b39fbe4e96e6c9a7c6c63de6 |
| Remote tag | v0.7.97 |
| Tag peel SHA | 8ff34170fe98fd14cc1e10e30e95d842fe67f0f0 |

## Summary

B-direct mechanical refactor cycle. Closes
FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC by extracting the
2169-line inline `mod tests { ... }` block from
`crates/chronos-services/src/counterexample.rs` (lines 1786-3955)
into a sibling file
`crates/chronos-services/src/ce_services_tests.rs` using the
`#[path = "..."]` submodule pattern established by m9-94.

After m9-95: `counterexample.rs` shrinks from 3955 → 1794 lines
(production code only); `ce_services_tests.rs` holds the 45 test
functions + 4 test helpers in a sibling submodule reachable as
`counterexample::tests`. The Rust module graph is identical pre- and
post-extraction (same items, same visibility, same resolution paths).

After m9-95: **cc-001-god-module** is now **fully closed** (both
production halves closed by m9-84..m9-87; both test halves closed by
m9-94 + m9-95).

## Drift delta

None. m9-95 is a Rust-only cycle; no vault CC drift introduced or
closed.

## Files changed

| Bucket | Files | Lines | Notes |
|---|---|---|---|
| crates/chronos-services/src/ce_services_tests.rs | 1 added | +2181 | NEW sibling file (verbatim copy of inline test block + 12-line doc comment header) |
| crates/chronos-services/src/counterexample.rs | 1 modified | -2169 / +11 | replaced 2170-line inline `mod tests { ... }` block with 11-line `#[path = "..."]` declaration |
| m9-95 cycle artifacts | 7 added | — | apply-checkpoint + 6 receipts/reports |
| m9-95 knowledge artifacts | 5 added | — | proposal + spec + tasks + exploration + change-entry |
| cycles/index.md | 1 modified | +2 | m9-95 row + Total 94 → 95 |
| terms/index.md | 1 modified | +1 | Last archive = m9-95-services-test-split |

**Total**: 2 source files modified + 12 cycle/knowledge artifacts + 2 index files.

## Cross-checks

- `bash scripts/check_vault_drift.sh`: PASS (48 python CCs + 7 bash CCs all clean).
- `python3 scripts/regen_manifest_index_shas.py --check`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p chronos-services --lib --no-fail-fast`: 268 pass.
- `cargo test --workspace --lib --no-fail-fast -- --test-threads=1`: 1042 pass (same as m9-94 baseline).
- `apply-checkpoint.peel_match == true`.
- `apply-checkpoint.head_sha == release-receipt.Head SHA == 8ff34170`.
- `Remote tag` v0.7.97 peel: `8ff34170` (cycle-artifacts commit; tag pre-created at cycle-artifacts per CC#42 workaround).
- `apply-checkpoint.status == "CLOSED"`.
- `apply-checkpoint.archive_status == "complete"`.
- `apply-checkpoint.findings_introduced.no_action == []` (cc#19-compliant).
- `cycles/index.md` Total cycles = 95 (matches actual folder count).
- `terms/index.md` Last archive = m9-95-services-test-split.
- No new tests added (45 tests + 4 helpers moved verbatim).
- No regressions in 1042 lib tests.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-95-services-test-split/apply-checkpoint.json` | `c95306842c851428a24ae8da2fc8f99a78da824c8cec3770f95cac026c3d3fb7` |
| implementation-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-95-services-test-split/implementation-receipt.md` | `6aa5ce4a26063022563130aec952b371dd3edf827ff0544e86efc40556fb9de0` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-95-services-test-split/merge-receipt.md` | `d061665dec0e5fe9313e1643dadbb62563058fc6ac352f55379c17236cc035b6` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-95-services-test-split/release-receipt.md` | `00417bef70eeb4f87f999ab38115950f444f6a915df234866ba3be3ac74592a6` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-95-services-test-split/release-report.md` | `9797c10346f20694b4f918896796ce65fee5daaca49a694a30594394b42ff1f2` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-95-services-test-split/verify-findings.json` | `b70497578acad2b048958c62de7f97540b4abd3382a94fe7ae72ceb7ff33bc6f` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-95-services-test-split/verify-report.md` | `8b5c2a3c53e5752b375e4a3bd0d4ef1bbcad4548e8f0ed60c2b6bcb7a9f78a11` |
| proposal | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-95-services-test-split/proposal.md` | `0a9fdf84af7d8d403a050cf14190074d5acd7a0168d35cd1a8d8becdad4d9713` |
| spec | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-95-services-test-split/spec.md` | `db05a6eabfdc3548518bdd6a35bcd17a43622ebee5ca7842c120300a2b96e442` |
| tasks | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-95-services-test-split/tasks.md` | `f9410ac784d1fb53f8c9afa2c8baa7491a3480d619117f483956dca1b11d3e0c` |
| exploration-report | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-95-services-test-split/exploration-report.md` | `132a2f5bb02309c3d17dcc4257a94a048ccaadc2bbce0e1a721cb7dda6fbc5fb` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-95-services-test-split/change-entry.md` | `a95f964c01067fe3a207486b4c9482b64b4c8fa54788382ed8f83c5816aefa25` |

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
| Source commit (Rust) | `eb96861b17711f7d525cedacccc17905bc17e6e0` (m9-95: extract services tests) |
| Cycle artifacts commit | `8ff34170fe98fd14cc1e10e30e95d842fe67f0f0` (m9-95: cycle artifacts + knowledge files) |
| SHA-cascade commit | `38fba83c67c3985f85df190fb3c72259d035b8a6` (m9-95: align artifacts to cycle-artifacts SHA) |
| Index-cascade commit | `3a195111e26e6cd3b39fbe4e96e6c9a7c6c63de6` (m9-95: add cycle row + bump Total to 95) |
| Merge commit | `a33b84768026044b4500d677793b5654c410887b` (--no-ff merge into main) |
| Merge-receipt-fill commit | `a5fa9333b759ed3f27339d70fcb2ea04727bd97f` (m9-95: fill merge SHA in merge-receipt.md) |
| Tag v0.7.97 | `8ff34170` (pre-created at cycle-artifacts per CC#42 workaround) |
