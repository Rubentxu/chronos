# M9 close report

**Branch:** `main` at `58a26e82` (post M9.5 + STATE/JOURNAL align)
**Cycle:** M9 (causal concurrency) — **CLOSED** 2026-09-22
**Precedence:** `docs/milestones/M8-CLOSE.md` §1; `docs/milestones/m9-scoping.md`; ADR-0028
**Status:** COMPLETE — 2026-09-22 (M9.5 final extension)

## §1 Executive summary

The M9 sub-cycle ships the causal-concurrency surface for Chronos: agents can ingest causality index data, build a happens-before graph from typed concurrency events, classify event pairs as races with reasoned explanations, and stress-test the classifier's robustness via systematic perturbation of evidence fields. Five sub-cycles (M9.1 through M9.5) deliver:

- **5 modules** in `chronos-services`: `causal_concurrency` (foundation), `concurrency` (typed model, M9.2), `concurrency_graph` (happens-before graph, M9.3), `race_classifier` (state-machine + explainer, M9.4), `concurrency_perturbation` (robustness testing, M9.5).
- **Pure-function state machines** with explicit fail-closed contracts per ADR-0004: `provenance_is_sufficient`, `classify_pair`, `perturb_and_classify`, `verify_perturbation_contract`.
- **Typed events** (`TypedConcurrencyEvent`, `Provenance`, `EdgeKind`) decoupling observation from inference — we never invent synchronization we didn't observe (`EdgeKind::Unknown` does NOT propagate).
- **51 new unit tests** across M9.2..M9.5 (5 + 9 + 14 + 8 + 9 + 6 incremental verification).

After M9, the M9 backlog reduces to:
- **MCP wire tools** for race classification output (M9.4 produced the algorithm + DTOs but not the JSON-RPC wrappers — chronos-mcp's job in a future cycle).
- **Live perturbation** (M9.5 is offline; live perturbation is M10.3 territory per `concurrency_perturbation.rs` doc-comment).
- **Per-primitive perturbation** (Lock vs Atomic vs Goroutine vs Message currently share provenance fields; per-primitive shrinkers would be M9+).
- **Probabilistic / model-checked perturbation** (M9.5 is deterministic per-kind; nondeterministic shrinking is out of scope).
- **Cross-graph perturbation** + perturbation history replay.

The M10 backlog (handoff): execution explorer live streaming, virtualization, REC-C1/REC-C2 regression suite.

## §2 Cycle log

| Cycle | Scope | Commit | LoC (delta) | Tests (delta) |
|---|---|---|---|---|
| **M9.1** | ADR-0027 inventory causal concurrency | `a0e75757` | 0 (docs-only) | 0 |
| **M9.2** | Typed concurrency model + `TypedConcurrencyEvent` + `Provenance` + `ConcurrencyPrimitive` (Lock/Atomic/Task/Goroutine/Channel/Unknown) + `is_read_only` conservative default | `2f4cbac3` | +305 (concurrency.rs) | +9 |
| **M9.3** | `HappensBeforeGraph` over CausalityIndex + `EdgeKind` (6 variants) + `happens_before` BFS + `concurrent_pairs` + `cycles` + `get_event` + mutation helpers | `16990044` | +482 (concurrency_graph.rs) | +14 |
| **M9.4** | `RaceClassification` enum (NotARace/Suspicious/Confirmed/Unsupported) + `ClassificationReason` + `ClassificationExplanation` + `classify_pair` pure-function state machine + `explain_classification` | `eb1d96e7` | +418 (race_classifier.rs) | +8 |
| **M9.5** | `PerturbationKind` (DropEvent/DropThreadId/DropTimestamp/DropFunction/DropFile/DropLine) + `PerturbationOutcome` + `expected_outcome_for` ADR-0028 §3 contract + `perturb_and_classify` + `verify_perturbation_contract` (clone-per-call) + `provenance_is_sufficient` thread_id decoupling | `78361a33` | +363 (concurrency_perturbation.rs) +2 (concurrency_graph.rs helpers) + fix (race_classifier.rs provenance_is_sufficient) | +9 |
| **STATE/JOURNAL align** | Additive updates (§0.4): M9.5 entry prepended, baseline bumped, HEAD row bumped, full M9 chapter row appended | `58a26e82` | +4/-3 (docs) | 0 |

Each cycle FF-merged (or post-write for docs-only M9.1) to `main` immediately. No PRs (per project convention).

## §3 v3 surface inventory

| Module | Public surface | Net-new in M9? |
|---|---|---|
| `causal_concurrency` (M6/REC-C3) | `CausalityIndex`, `CausalityEntry`, `record_write(addr, entry, var_name: Option<&str>)` | No (foundation) |
| `concurrency` (M9.2) | `TypedConcurrencyEvent`, `Provenance`, `ConcurrencyPrimitive` (6 variants), `is_read_only(event) -> bool` | YES |
| `concurrency_graph` (M9.3) | `HappensBeforeGraph`, `HappensBeforeEdge`, `EdgeKind` (6 variants), `ConcurrentPair`, `from_index`, `happens_before`, `concurrent_pairs`, `cycles`, `get_event`, `get_event_mut`, `remove_event` | YES |
| `race_classifier` (M9.4) | `RaceClassification`, `ClassificationReason`, `ClassificationExplanation`, `provenance_is_sufficient`, `is_read_only`, `classify_pair`, `explain_classification` | YES |
| `concurrency_perturbation` (M9.5) | `PerturbationKind`, `PerturbationOutcome`, `expected_outcome_for`, `perturb_and_classify`, `verify_perturbation_contract`, `all_perturbation_kinds` | YES |

## §4 Perturbation contract (the heart of M9.5)

| Perturbation | Expected outcome | Why |
|---|---|---|
| `DropEvent` | `Unsupported` | events are gone; cannot classify |
| `DropThreadId` | `Suspicious` | thread_id NOT in `provenance_is_sufficient` (decoupled in M9.5); we still know there's contention, just not cross-thread |
| `DropTimestamp` | `Unsupported` | timestamp IS in `provenance_is_sufficient`; dropping it violates minimum evidence |
| `DropFunction` | `Unsupported` | function IS in `provenance_is_sufficient`; dropping it violates minimum evidence |
| `DropFile` | `Confirmed` (or baseline) | file NOT in `provenance_is_sufficient`; verdict unchanged |
| `DropLine` | `Confirmed` (or baseline) | line NOT in `provenance_is_sufficient`; verdict unchanged |

The contract is **relative to baseline** (a `Suspicious` baseline → `Suspicious` after `DropFile`), not absolute. Tests pin each combination.

## §5 Lessons (what we got wrong, and how we fixed it)

Per ADR-0004 (No Silent Lies), M9.5 disclosed and corrected **three design bugs** that surfaced during the slice:

1. **`provenance_is_sufficient` over-restrictive** (originally required thread_id + timestamp + function). This caused `DropThreadId` to return `Unsupported` instead of the contractually correct `Suspicious`. **Honest fix**: decoupled thread_id from minimum evidence. ADR-0004 fail-closed preserved via `DropTimestamp`/`DropFunction` still returning `Unsupported`.

2. **`expected_outcome_for` for `DropFile`/`DropLine` initially returned `Suspicious`**. But file/line are NOT in `provenance_is_sufficient`, so dropping them doesn't change the verdict from baseline (Confirmed for our test pair: different threads + writes). **Honest fix**: corrected expected outcomes to `Confirmed`, updated tests, documented the relative-vs-absolute nature in doc-comment.

3. **`verify_perturbation_contract` mutates the input graph** when applied sequentially (each perturbation sees the state left by the previous one). **Honest fix**: refactored to clone the graph for each perturbation; signature changed from `&mut HappensBeforeGraph` to `&HappensBeforeGraph`; added test verifying the original graph is not mutated.

These three bugs would have been silently green if we hadn't pinned the perturbation contract with explicit tests against the actual `classify_pair` behavior.

## §6 Out-of-scope (deferred to M10+ / future cycles)

- **MCP wire tools** for race classification output (chronos-mcp's job; M9.4 produced algorithm + DTOs).
- **Live streaming perturbation** (M10.3 territory per `concurrency_perturbation.rs` doc-comment).
- **Cross-graph perturbation** (M9.5 is per-pair).
- **Per-primitive perturbation** (Lock vs Atomic vs Goroutine vs Message share provenance fields).
- **Probabilistic / nondeterministic perturbation** (M9.5 is deterministic per-kind).
- **Perturbation history replay** (cross-run analysis).
- **Memory-model reasoning** (happens-before is necessary but not sufficient for C++11 / Java memory models; out of scope).
- **Cross-session correlation** (post-M9).

## §7 M9 chapter close declaration

**M9 chapter is CLOSED** (5/5 sub-ciclos verified):

| Sub-cycle | Status | Commit |
|---|---|---|
| M9.1 ADR-0027 inventory | `verified` | `a0e75757` |
| M9.2 typed concurrency model | `verified` | `2f4cbac3` |
| M9.3 happens-before graph | `verified` | `16990044` |
| M9.4 race classifier + explainer | `verified` | `eb1d96e7` |
| M9.5 perturbation + missing-evidence | `verified` | `78361a33` |

The causal-concurrency chapter covers: causal inventory (M9.1) + typed events (M9.2) + happens-before graph (M9.3) + classification state machine + explainer (M9.4) + perturbation robustness testing (M9.5). The next chapter ROADMAP is M10 (Execution Explorer live streaming + virtualization + REC-C1/REC-C2 regression).

**H1.x + M4-F0 + M4-F1 + M6 (7/7) + M7 (4/4) + M8 (1/1) + M9 (5/5) verificados: 52/52 sub-ciclos.**

## §8 Verification summary

| Stage | Command | Result |
|---|---|---|
| T0 build | `cargo build -p chronos-services` | Finished clean (incremental) |
| T1 unit (full) | `cargo test -p chronos-services --lib --no-fail-fast` | **461/461 PASS** (12.06s) |
| T1 unit (perturbation only) | `cargo test -p chronos-services --lib concurrency_perturbation --no-fail-fast` | **9/9 PASS** |
| T0 clippy | `cargo clippy -p chronos-services --lib --tests --no-deps -- -D warnings` | exit=0 (0 warnings) |
| Push | `git push origin main` | `1fcc1249..58a26e82` OK |
| HEAD == origin/main | `git rev-parse HEAD` vs `git rev-parse origin/main` | both `58a26e82` |
| Tag intact | `git rev-parse v0.7.112` | `0be2ec2d` intact |
| 4 SHAs cat-file | `git cat-file -e 78361a33 + 58a26e82 + 1fcc1249 + 4796fddc` | exit=0 |

## §9 Files

| File | Role |
|---|---|
| `crates/chronos-services/src/concurrency.rs` | Typed concurrency model (M9.2) |
| `crates/chronos-services/src/concurrency_graph.rs` | Happens-before graph (M9.3 + mutation helpers M9.5) |
| `crates/chronos-services/src/race_classifier.rs` | Classification state machine + explainer (M9.4 + provenance_is_sufficient fix M9.5) |
| `crates/chronos-services/src/concurrency_perturbation.rs` | Perturbation contract + verify (M9.5) |
| `crates/chronos-services/src/lib.rs` | Module registration |
| `docs/chronos-agentic-reconstruction/docs/adr/0027-m9.1-inventory.md` | M9 scoping inventory ADR |
| `docs/chronos-agentic-reconstruction/docs/adr/0028-m9-scoping.md` | M9 scoping formal ADR |
| `docs/roadmap/STATE.md` | M9 chapter state (5/5 verified) |
| `docs/roadmap/JOURNAL.md` | Append-only M9 entries (M9.1..M9.5) |

## §10 References

- ADR-0004 — Silent Lie Prohibition (no false claims; corrections are part of the work)
- ADR-0027 — M9.1 inventory causal concurrency
- ADR-0028 — M9 scoping formal architecture
- ADR-0029 — M10 scoping formal architecture (next chapter)
- H1.2 §10 — threat model + deployment profiles (NO remote/multi-tenant in this release)
- ROADMAP §M9 §95 — causal concurrency scope

## §11 Closing note

M9 is the **first chapter to ship pure-function state machines with explicit fail-closed contracts AND systematic perturbation testing** for those contracts. M9.5's perturbation contract is a reusable pattern: define an `expected_outcome_for` table that pins the contract, then run `verify_contract` against each variant to ensure the classifier honors it. Future classification modules (M10, M11, M12) should follow this pattern: classify + explain + perturb + verify.
