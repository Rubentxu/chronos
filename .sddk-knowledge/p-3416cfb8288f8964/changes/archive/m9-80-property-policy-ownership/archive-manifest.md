# Archive Manifest — m9-80-property-policy-ownership

## Summary

m9-80 closes the property-policy ownership refactor: the four primitives
`eval_invariant`, `eval_existence`, `eval_call_path`, and
`observe_property_target` that used to live as private helpers in
`chronos_services::hypothesis_test` are now public APIs in
`chronos_domain::property`. The refactor uses a layered split (spec rev 2)
that avoids the dependency cycle a literal byte-for-byte move would have
caused: domain owns the policy semantics and returns domain-owned outcome
types (`InvariantOutcome`, `ExistenceOutcome`, `CallPathOutcome`,
`PropertyHypothesisVerdict`, `PropertyObservationSource`,
`PropertyExistencePredicate`, plus the family umbrella
`PropertyHypothesisOutcome`); services owns the wire shape
(`HypothesisOutput`, `HypothesisVerdict`, `ExistencePredicate`,
`HypothesisScope`) and wraps the domain outcomes via 6 new `From` impls.

The `HypothesisTest::test` signature is unchanged
(`pub async fn test(ctx, input) -> Result<HypothesisOutput, ServiceError>`).
No MCP schema, no JSON envelope, no `session_*` action signature change.
The wire shape is preserved exactly through the From impls.

The 13 existing `hypothesis_test` unit tests pass without modification of
their assertions — the test bodies may have gained `use` imports to reach
the domain API, but every assertion string and int is preserved. This is
the strongest evidence that the refactor is byte-equivalent in policy
semantics.

The cycle is part of the m9-roadmap. m9-80 closes one of the longest-pending
refactors identified during the m0-truth-first-foundation cycle (see
`feat/m0-truth-first-foundation` branch history). It unblocks:

- A real `evaluate_hypothesis` MCP tool (deferred to a future cycle; the
  underlying capability is reachable today via `session_explain{kind=hypothesis}`).
- A `PropertyObserver` trait generalisation of `observe_property_target`
  (deferred until a non-hypothesis caller appears).

No deferred findings introduced. No new tests added (all 13 hypothesis_test
unit tests pre-existed; the cycle is a pure refactor).

## Cycle

| Campo | Valor |
|---|---|
| Cycle | `m9-80-property-policy-ownership` |
| Path | A-min |
| Status | CLOSED |
| Base SHA | `82e219f812655a136e736f443ec8b86695050110` |
| Head SHA | `8012342534f33b92b9edd8002cda3cf194937fa6` |
| Branch | `feat/m9-80-property-policy-ownership` |
| Tag | `v0.7.82` |
| Route | A-min |
| Merge SHA | `7874e5c8e972172c024b07f60746f0e06df92f9d` |
| Merge type | `--no-ff` (preserved cycle branch topology) |
| Date | 2026-09-14 |

## Deliverables

| Artifact | Kind |
|---|---|
| `crates/chronos-domain/src/lib.rs` | extends `pub use property::{…}` block at line 28 with the 7 new domain-owned types |
| `crates/chronos-domain/src/property.rs` | +777/-165: 7 new types + 4 new pub fns + private `bfs_reach_domain` BFS helper |
| `crates/chronos-services/src/hypothesis_test.rs` | -239/+35: removes 10 services-layer private fns; 4 match arms delegate to domain |
| `crates/chronos-services/src/output.rs` | +20: 6 new `From` impls for the property-policy outcome types |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-80-property-policy-ownership/T0-progress-note.md` | T0 progress note (relocated from cycle-artifacts/ to changes/ to satisfy CC#18) |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-80-property-policy-ownership/change-entry.md` | this cycle's change-entry (created at archive phase) |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-80-property-policy-ownership/archive-manifest.md` | this file |
| `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | adds m9-80 row at line 104; Total cycles 80 → 81 |
| `.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-backlog-blocked-2026-09-12.md` | appends T0/T1/T2/T3/T4/T5 handoff sections (the repeatable T1→T5 pattern) |
| `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` | Last updated timestamp refresh |
| `cycle-artifacts/p-3416cfb8288f8964/m9-80-property-policy-ownership/{apply-checkpoint.json,implementation-receipt.md,merge-receipt.md,release-receipt.md,release-report.md,verify-findings.json,verify-report.md}` | the 7 cycle artifacts required by CC#30/CC#36/CC#51 |

## Evidence bindings

- **`apply-checkpoint.json`**: status CLOSED; verify_status passed; release_status released; archive_status archived (after this commit); `tag = "v0.7.82"`, `tag_peel_sha = main_sha = merge_commit_sha = 7874e5c8e972172c024b07f60746f0e06df92f9d`.
- **`verify-findings.json`**: 0 findings (empty `findings: []`); `subject.head = "ccf8811"` (the cycle HEAD when verify ran; HEAD before merge); `subject.verdict = "passed"`; `lens_summary` populated (≥200 chars).
- **`verify-report.md`**: Verdict PASS, 0 critical / 0 warnings; `## Files Inventory`, `## Cross-checks`, `## Behavioral Compliance`, `## Spec Mechanical Scenarios`, `## SOLID And Design`, `## Architecture Delta` all present.
- **`merge-receipt.md`**: Base SHA `82e219f`, merge `7874e5c` (--no-ff), `HEAD == origin/main == v0.7.82^{commit} == 7874e5c`.
- **`release-receipt.md`**: `Remote tag | v0.7.82`, peel `7874e5c8e972172c024b07f60746f0e06df92f9d`, Peel match = true (clean: tag_peel == merge_commit_sha).
- **`gate-implementation-complete-…`** (passed): T0..T5 build phase complete (commit `ccf8811`).
- **`gate-tests-pass-…`** (passed): verify phase complete (T0 + T1 + T2 + T3 + T4-smoke all green).
- **`gate-policy-compliant-…`** (passed): verify phase complete (all 6 spec mechanical scenarios PASS).
- **`gate-no-pending-effects-…`** (passed): release phase complete (merge `7874e5c` + tag `v0.7.82` published).
- **`gate-release-uat-approved-…`** (passed): release phase complete (release-receipt.md, merge-receipt.md, release-report.md, apply-checkpoint.json all populated).

## Falsification evidence

The cycle is a pure refactor. Falsification reduces to "revert one of the
6 From impls (or one of the 4 domain fns), observe the test fail or the
crate not compile, restore, re-run green":

| Reverted | Observed result |
|---|---|
| `impl From<InvariantOutcome> for HypothesisOutput` removed from `output.rs` | compile error: `HypothesisOutput::from(outcome)` in `hypothesis_test.rs:139` unresolved |
| `impl From<ExistenceOutcome> for HypothesisOutput` removed | compile error in `hypothesis_test.rs:171` |
| `impl From<CallPathOutcome> for HypothesisOutput` removed | compile error in `hypothesis_test.rs:184` |
| `chronos_domain::property::eval_invariant` removed | compile error: `eval_invariant` not found in `hypothesis_test.rs:133` |
| `chronos_domain::property::eval_existence` removed | compile error in `hypothesis_test.rs:169` |
| `chronos_domain::property::eval_call_path` removed | compile error in `hypothesis_test.rs:178` |
| `Property` removed from `pub use property::{…}` block at `lib.rs:30` | compile error: any downstream `use chronos_domain::Property` would fail |
| `fn eval_invariant` restored in `hypothesis_test.rs` (dead) | `#[allow(dead_code)]` warning + the match arm still uses domain fn (no behavioural change) — but the spirit of the refactor is broken: there are now two `eval_invariant` symbols and the services-layer one is unused |

The 13 hypothesis_test unit tests cover the policy semantics end-to-end.
Passing them after the refactor (without changing their assertions) is the
strongest behavioural-equivalence evidence.

## Tangential modifications

- **Vault indexes**: `cycles/index.md` gained the m9-80 row (line 104);
  Total cycles 80 → 81. `terms/index.md` and `maintenance/vault-drift-sweep.md`
  gained `Last updated` timestamp refreshes.
- **CC#4 cascade**: 10 `archive-manifest.md` files (m9-02, m9-68, m9-70,
  m9-71, m9-72, m9-73, m9-74, m9-75, m9-76, m9-79) had their index SHA rows
  regenerated to a fixpoint by `scripts/regen_manifest_index_shas.py` after
  the m9-80 row landed in `cycles/index.md`. The self-referential rows of
  pre-m9-11 manifests and m9-67/m9-68 are preserved by design.
- **Handoff**: 6 new sections appended to
  `m9-backlog-blocked-2026-09-12.md` describing the T1 → T5 repeatable pattern
  (move one fn + 1-N From impls + delegation + dead-helper cleanup + verify).

## Cross-checks

- CC#1..CC#56: pass (48 python + 7 bash, counts unchanged post-cycle).
- CC#4: artifact index SHA-256 consistency satisfied by the regen tool
  (`scripts/regen_manifest_index_shas.py`); the post-archive commit runs
  it to fixpoint.
- CC#12: `main_sha == head_sha == remote_tag_peel == 7874e5c8e972172c024b07f60746f0e06df92f9d`.
- CC#21 (`## Evidence bindings` in archive-manifest): this section.
- CC#24 / CC#31 (`## Cross-checks`): present in `change-entry.md`,
  `verify-report.md`, and this manifest.
- CC#33 (`## Subject`): present in `change-entry.md`, `verify-report.md`,
  `verify-findings.json`.
- CC#36 / CC#55 / CC#39: `verify-report.md` carries `lens_summary`, `## Files Inventory` and the canonical summary table.
- CC#42: peel-match recorded in `release-receipt.md` (clean: `v0.7.82 → 7874e5c`).
- CC#51: cycle-artifacts folder exists for the m9-80 row.
- T0: `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
- T1: `cargo test -p chronos-services -p chronos-domain --lib --no-fail-fast` → 264 + 149 = 413 passed.
- T2 (filter): `cargo test -p chronos-services --lib hypothesis_test --no-fail-fast` → 13/13 (67.31 s).
- T3 (native serial): `cargo test -p chronos-native --lib --no-fail-fast -- --test-threads=1` → 103/103 (13.03 s).
- T4-smoke (`--test-threads=1`, `CHRONOS_MCP_PATH` exported):
  - `chronos-sandbox/tests/session_lifecycle::*` → 4/4 (28.57 s)
  - `chronos-sandbox/tests/analytics_tools::*` → 11/11 (79.89 s)
  - `chronos-sandbox/tests/program_scenarios::*` → 8/8 (58.32 s)

## Follow-ups (deferred)

None new from this cycle. m9-80 closes the property-policy ownership
refactor without introducing debt. Preserved low-severity items remain
unchanged:

- **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE** (low)
- **FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL** (low)
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low)
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low, mitigated)
- **FIND-M9-66** (broken JSON in m9-66 verify-findings.json)
- **Pre-existing smoke-test work-copy isolation flake** (do not use
  `scripts/smoke_test_ccs.sh` as a CI gate until the leak is fixed).

The spec explicitly defers (carry-forward, not findings):

- A real `evaluate_hypothesis` MCP tool (current docstring at
  `crates/chronos-mcp/src/server.rs:4949` is misleading but the underlying
  capability is reachable via `session_explain{kind=hypothesis}`).
- Generalisation of `observe_property_target` into a `PropertyObserver`
  trait (defer until a non-hypothesis caller appears).

Next roadmap candidate: continue m9-roadmap. The property-policy surface
is now stable enough that an MCP tool can be added with low risk. See the
m9-roadmap project issue for the prioritized list.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-80-property-policy-ownership/archive-manifest.md` | `91147d4716cfaeb89a0f868cc18956e2d3f6fe77f2717ba3f8aad15711d22353` |
| implementation-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-80-property-policy-ownership/implementation-receipt.md` | `dc9de098846a3e35df5f810defc7ee3f86fbeb0e458c48f3a219527ea3500968` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-80-property-policy-ownership/verify-report.md` | `40bec32a8f4885102a1c0dc789722e38e4e13b6b00771f46ef18a118c6438645` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-80-property-policy-ownership/verify-findings.json` | `bd427da3396f105cd02c462126134c9d8efd154824e7d5af14ca4a1c4dcf42ac` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-80-property-policy-ownership/release-receipt.md` | `e30e225ab0680c57081276025643aecfd9c474092edd4b4846e3ea67fa2d1c17` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-80-property-policy-ownership/merge-receipt.md` | `27fd81a15d1e42154a6dd27fd1a70218417240494e53f29336701be15729c8b3` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-80-property-policy-ownership/release-report.md` | `94a740505682c1ef639e6217b95a9f2679a68312d9a39dbd431b86902ebe1630` |
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-80-property-policy-ownership/apply-checkpoint.json` | `de751090752543c2301a3c7e5a410c0c0c9aa0199bbc1e486907899f41375790` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-80-property-policy-ownership/change-entry.md` | `a759367b8e085afeb3d2dcd3a0abbb71f7b72c9638651f632726733ee18ef1ff` |
| vault index (cycles) | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | `2ccfbb229fe92af1f647440596a5bc4ee76a55571949ede31bd3c56b25bad5df` |
| vault index (terms) | `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | `befab10ccae378abd04549f0e1316c38cc93157509e62a1e5d5c1edc71c9ccdc` |
| source (domain lib) | `crates/chronos-domain/src/lib.rs` | (8 entries added to `pub use property::{…}` block at line 28) |
| source (domain property) | `crates/chronos-domain/src/property.rs` | (+777/-165: 7 new types + 4 new pub fns + private `bfs_reach_domain` helper) |
| source (services hypothesis_test) | `crates/chronos-services/src/hypothesis_test.rs` | (-239/+35: removed 10 private fns; 4 match arms delegate to domain) |
| source (services output) | `crates/chronos-services/src/output.rs` | (+20: 6 new `From` impls) |
