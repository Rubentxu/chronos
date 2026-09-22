# M8-07 merge doc — Hypothesis reconstruction fidelity

**Cycle:** `m8-07-hypothesis-reconstruction-fidelity`
**Branch:** `feat/m8-07-hypothesis-reconstruction-fidelity` (deleted by CC#53 cleanup; code FF-merged before cleanup)
**Base:** `148f009a` (M8 chapter close)
**Head:** `bceddc94` (this cycle's last commit)
**Tag:** `m8-07-hypothesis-reconstruction-fidelity.0` (NEW, post-hoc, marks the closed cycle)
**Status:** COMPLETE — 2026-09-11 (originally); close-of-record documented 2026-09-22 (M8.7.1)

## §1 What this cycle ships

m8-07 closes **m8-04 R-hypothesis-reconstruction-fidelity** (deferred from m8-04 per its close report). The original m8-04 `chronos test replay <bundle_id>` reconstructed a `HypothesisInput` from bundle summary fields + engine state, which was lossy when bundles were loaded outside the originating session. m8-07 closes that gap by **persisting the originating `target_hypothesis` inside the bundle record** so replay reconstructs exactly what the agent passed at shrink time.

| Deliverable | B-item | What |
|---|---|---|
| `HypothesisInputWire` field-for-field mirror in `chronos-store` | B1.1 | stringified enums for `kind` / `scope` / `comparison` to keep the wire shape stable |
| `CounterexampleBundleRecord.target_hypothesis: Option<HypothesisInputWire>` with `#[serde(default)]` | B1.2 | backward-compatible with pre-m8-07 bundles (deserialize as `None`); replay falls back to m8-04 synthetic reconstruction |
| `counterexample_shrink()` propagates `target_hypothesis: &HypothesisInput` to `save()` | B1.3 | round-trip closure — what the agent passes is what gets persisted |
| `save()` persists `Some(hypothesis_input_to_wire(target_hypothesis))` | B1.4 | wire encoding helper (mirror of `from_wire` already in m8-04 scope) |
| `reconstruct_hypothesis` reads `bundle.target_hypothesis` first, falls back to m8-04 synthesis | B2.1 | path 1 = exact reconstruction; path 2 = legacy fallback |
| `crates/chronos-cli/src/replay.rs` uses persisted `target_hypothesis` | B2.2 | CLI `chronos test replay <bundle_id>` now round-trips the originating invariant |
| Sandbox test `ce12_replay_preserves_non_default_invariant_options` | B3.1 | end-to-end wire smoke verifying replay preserves non-default options |
| Sandbox CLI plumbing + `process.rs` support for replay | B3.2 | runtime path for `chronos test replay` |

## §2 Cycle commits (chronological)

| Commit | Subject | Files / LoC |
|---|---|---|
| `4338aa26` | docs(milestones): m8-07 hypothesis reconstruction fidelity scoping (PROPOSED) | `docs/milestones/m8-07-hypothesis-reconstruction-fidelity-scoping.md` +335L |
| `12489948` | feat(m8-07): HypothesisInputWire + target_hypothesis in CounterexampleBundleRecord | `crates/chronos-store/src/counterexample_storage.rs` +141L |
| `fff77da8` | feat(m8-07): hypothesis_input_to_wire/from_wire + save/shrink plumbing | `crates/chronos-services/src/counterexample.rs` +341L / -X |
| `2e346ce6` | feat(m8-07): reconstruct_hypothesis uses persisted target_hypothesis | `crates/chronos-cli/src/replay.rs` +128L / -X |
| `45682469` | feat(m8-07): sandbox infrastructure + ce12 replay test | `chronos-sandbox/src/client/tools.rs` +142L; `chronos-sandbox/tests/counterexample_tools.rs` +124L |
| `bceddc94` | docs: m8-07 apply-checkpoint | `sddk/changes/m8-07-hypothesis-reconstruction-fidelity/apply-checkpoint.json` +102L |

**Total:** 8 files changed, +1,329 insertions, -12 deletions across 6 commits.

## §3 Architectural decisions (per-cycle summary)

These are summarised from `docs/milestones/m8-07-hypothesis-reconstruction-fidelity-scoping.md` and recorded here for traceability:

- **A1 — `target_hypothesis` is canonical, not reconstructed.** Replay must reconstruct exactly what the agent passed at shrink time, not a best-effort synthesis from bundle summary fields. (m8-04 R-hypothesis-reconstruction-fidelity closes here.)
- **A2 — `Option<HypothesisInputWire>` with `#[serde(default)]`.** Pre-m8-07 bundles (m8-01..m8-06 era) load as `target_hypothesis: None` and fall back to m8-04 synthetic reconstruction. Zero migration cost; no bundle becomes unloadable.
- **A3 — Stringified enums in the wire shape.** `kind` / `scope` / `comparison` are persisted as `String` rather than re-using the typed `HypothesisKind` enum. This decouples the wire from the rust enum's internal naming and is consistent with m8-04's `HypothesisInputWire` precedent.
- **A4 — Path 1 (persisted) takes priority over path 2 (synthesis).** If a bundle has `target_hypothesis = Some(...)`, replay uses it verbatim. Only if it's `None` does the legacy synthesis path run. This is the safest ordering — silent fallback to synthesis could mask data corruption in the persisted path.
- **A5 — Sandbox `ce12` as the only new test in this cycle.** The 4 deliverables (B1-B3) are testable via existing ce1..ce11 + the new ce12. ce12 specifically targets "replay preserves non-default invariant options", which is the visible surface of B2.

## §4 Deviations from scoping

None. The scoping doc (`docs/milestones/m8-07-hypothesis-reconstruction-fidelity-scoping.md`) and the apply-checkpoint (`sddk/changes/m8-07-hypothesis-reconstruction-fidelity/apply-checkpoint.json`) agree on scope and deliverables.

## §5 Verification (as executed at close time)

Per `apply-checkpoint.json`:

- **T0** `cargo clippy --workspace --all-targets --no-deps -- -D warnings` exit=0 (chronos-store + chronos-services + chronos-cli + chronos-sandbox ce12 path).
- **T1** `cargo test -p chronos-services --lib` — counterexample.rs tests including `reconstruct_hypothesis` path green.
- **T2** `cargo test -p chronos-cli --test replay` — replay round-trip green.
- **T3** `cargo test --test counterexample_tools -- ce12` (sandbox) — `ce12_replay_preserves_non_default_invariant_options` PASS.
- **CC#4** archive manifests clean (post-merge of m8-07 commits).

## §6 Why m8-07 was un-tagged until 2026-09-22

Per ADR-0026 §7 (M8.1 inventory, 2026-09-22): the original M8 chapter close (`148f009a`) wrapped m8-06 as the last formal sub-cycle. m8-07's `apply-checkpoint.json` recorded `status: implemented` with `merge_sha: null` — the code was FF-merged to `main` and the branch was cleaned by CC#53 (m9-65/66 stale branch sweep), but no formal merge commit was authored, and no tag was created. This is consistent with the project's "FF-merge + bookkeeping commit" convention for most cycles, but it left m8-07 discoverable only via grep on commit messages — not via `git tag`.

M8.7.1 (this slice, 2026-09-22) creates the missing close-of-record:

1. This merge doc (`docs/milestones/m8-07-hypothesis-reconstruction-fidelity-merge.md`) — formal merge report matching m8-01..m8-06 format.
2. Tag `m8-07-hypothesis-reconstruction-fidelity.0` → `bceddc94` (last commit of the cycle).
3. Update `sddk/changes/m8-07-hypothesis-reconstruction-fidelity/apply-checkpoint.json` with `merge_sha: bceddc94`.
4. STATE.md + JOURNAL.md alignment to reflect m8-07 close-of-record.

This brings m8-07 to parity with the other 6 m8-0X sub-cycles for traceability.

## §7 Out-of-scope (deferred)

- **Cross-variant existence predicate shrinking** — ExistencePredicateShrinker holds the variant fixed (m8-06 R4). Cross-variant shrinking would require generating arbitrary `ExistencePredicate` JSON. Significant scope expansion. (carried from m8-06 close report)
- **Bundle-as-blob → side table migration** — events currently ride inside the bundle blob. Splitting into a separate table is m9+ scope. (carried from m8-04 close report R4)
- **Bundle encryption / signature** — not in any cycle scope; deferred indefinitely.
- **Replay dry-run mode** — `chronos test replay <bundle_id>` always runs the hypothesis. A `--dry-run` flag would allow inspection without execution. Not in this cycle.

## §8 References

- `docs/milestones/m8-07-hypothesis-reconstruction-fidelity-scoping.md` — original scope.
- `sddk/changes/m8-07-hypothesis-reconstruction-fidelity/apply-checkpoint.json` — bookkeeping (post-M8.7.1 with `merge_sha`).
- `docs/milestones/m8-04-chronos-cli-engine-events-saved-rework-merge.md` §R-hypothesis-reconstruction-fidelity — the deferred close that m8-07 satisfies.
- ADR-0026 §7 — M8.1 inventory notes m8-07 as PARTIAL.
- 6 commits: `4338aa26`, `12489948`, `fff77da8`, `2e346ce6`, `45682469`, `bceddc94`.
- Tag: `m8-07-hypothesis-reconstruction-fidelity.0` → `bceddc94`.
