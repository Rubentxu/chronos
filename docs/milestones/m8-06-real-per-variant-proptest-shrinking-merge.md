# M8-06 merge doc — Real per-variant proptest shrinking

**Cycle:** `m8-06-real-per-variant-proptest-shrinking`
**Branch:** `feat/m8-06-real-shrinkers`
**Base:** `9b5eed0` (m8-06 scoping FF-merge)
**Head:** TBD (this execute commit)
**Tag:** `m8-06-real-shrinkers.0`
**Status:** COMPLETE — 2026-09-11

## §1 What this cycle ships

m8-06 closes one explicit deliverable from the scoping doc: **real per-variant
proptest shrinking** (B1). It closes m8-05 R3 ("per-variant strategy returns
`Just(base)`; real shrinking is m9+") by replacing the 3 stub helpers with real
`Strategy + ValueTree` implementations:

| Deliverable | B-item | What |
|---|---|---|
| NumberShrinker (binary search toward 0.0) | B1.1 | closes m8-05 R3 + R7 for `Invariant{constant: Number}` targets |
| TextShrinker (character deletion toward "") | B1.2 | closes m8-05 R3 + R7 for `Invariant{constant: Text}` targets |
| Bool stays `Just(b)` (degenerate) | B1.3 | honest disclosure R3 — only 2 values exist; shrinker is the identity |
| ExistencePredicateShrinker (variant fixed, payload shrinks) | B1.4 | closes m8-05 R3 for `Existence` targets |
| CallPathShrinker (round-robin caller/callee/max_depth shrinking) | B1.5 | closes m8-05 R3 for `CallPath` targets |
| `max_rounds` wired through to `proptest::Config::max_shrink_iters` | B1.6 | the m8-05 R9 disclosure ("`max_rounds` is only the infinite-loop guard") is now honoured; without this wire-up, every shrink call returns `rounds_used == 1` |

Also: 5 sandbox tests (ce1, ce5, ce7, ce10, ce11) re-wired to always-violating
hypotheses so the `rounds_used >= 2` assertion is meaningful.

## §2 Cycle commits

| Commit | Subject | Tests added |
|---|---|---|
| `9b5eed0` | Merge branch 'feat/m8-06-real-shrinkers' (scoping only) | — |
| TBD | feat(services+sandbox): real per-variant proptest shrinking (m8-06 execute) | +11 services unit + 5 sandbox rewires |
| TBD | chore(sddk): close m8-06 bookkeeping | — |

## §3 Architectural decisions (per-cycle summary)

These are summarised from `docs/milestones/m8-06-real-per-variant-proptest-shrinking-scoping.md`
and recorded here for traceability:

### D1 NumberShrinker — binary search toward 0.0

For `Invariant{constant: Number(n)}`, the shrinker takes `n` and walks toward
`0.0` via midpoint-then-zero. The shrinker tree is bounded by
`proptest::Config::max_shrink_iters` (default u32::MAX in proptest; we cap at
`DEFAULT_SHRINK_MAX_ROUNDS = 64` via the ShrinkConfig→proptest::Config mapping
added in this cycle).

### D2 TextShrinker — character deletion toward ""

For `Invariant{constant: Text(s)}`, the shrinker takes `s` and walks toward `""`
by deleting one character at a time (UTF-8 safe). Lexicographic — not byte-
optimal.

### D3 Bool stays `Just(b)`

`Bool` has only two values, so the only "shrinking" is the identity. Returning
`Just(b)` is correct; `rounds_used == 1` after the initial validation.

### D4 ExistencePredicateShrinker — variant fixed, payload shrinks

For `Existence` targets, the variant (`EventTypeEquals`, `PredicateEquals`,
etc.) is held fixed (R4 disclosure: cross-variant shrinking is m9+). The
payload string shrinks via the TextShrinker logic.

### D5 CallPathShrinker — independent field shrinking

For `CallPath` targets, the three fields (`caller`, `callee`, `max_depth`)
shrink in round-robin fashion. Each round picks one field and runs a single
simplify pass on it. Stack-safe via iterative loop (m8-05 R7 disclosure: dyn
ValueTree is !Send, so the tree cannot be held across `.await`; we rebuild it
each round).

### D6 `max_rounds` cap

`max_rounds` is now mapped to `proptest::Config::max_shrink_iters`. Without
this wire-up, the default u32::MAX would let any shrinking loop run forever
on a non-converging shrinker. The default value of 64 gives each shrinker ~64
binary-search iterations to converge.

## §4 Test pyramid state

- **Unit (chronos-services lib)**: 254/254 pass (was 243 pre-m8-06; +11).
- **Per-crate integration (T3)**: 1036 tests across `cargo test --workspace
  --lib --tests --exclude chronos-sandbox --exclude chronos-e2e --exclude
  chronos-native`. 0 failed, 12 ignored.
- **Sandbox smoke (T4)**: `counterexample_tools` 11/11 pass. `e2e_connectivity`
  and other suites not re-run because the change set is localised to the
  counterexample shrinker + sandbox wire JSON shape.

## §5 Disclosures

These are m8-06's open items; full text in apply-checkpoint.json. R6/R7/R8
carried unchanged from m8-05; R1..R5 + R9 are m8-06-specific.

| Risk | State | Plan |
|---|---|---|
| **R1**: `f64` shrinks toward `0.0` but cannot reach exactly `0.0` due to float rounding | accepted | use `EPSILON = 1e-9` comparison floor in shrinker; m9+ could swap in arbitrary-precision |
| **R2**: text shrinks lexicographically (delete one char at a time), not byte-optimally | accepted | proptest convention; m9+ can adopt dictionary-based shrinking for known token sets |
| **R3**: Bool shrinker is the identity (Just(b)) | accepted | only 2 values exist; degenerate by definition |
| **R4**: variant fixed in ExistencePredicateShrinker (no cross-variant shrinking) | open, m9+ | requires generating arbitrary `ExistencePredicate` JSON; significant scope expansion |
| **R5**: `max_depth = None` is a terminal state — cannot shrink past it | accepted | per proptest convention, terminal states are valid minima |
| **R6**: dyn ValueTree: !Send, strategy tree rebuilt each round | open, m9+ | path of least resistance; m9+ may add `ValueTree: Send` feature flag |
| **R7**: SBoxedStrategy requirement (m8-05 R5) | open, accepted | constraint, not a bug |
| **R8**: HypothesisInput: Send + Sync + 'static requirement (m8-05 R4) | open, m9+ | the path of least resistance — no decision needed yet |
| **R9** (NEW): `max_rounds` is the only infinite-loop guard on `drive_strategy` | accepted | the ShrinkConfig→proptest::Config mapping added in m8-06 wires this through; without it, the loop would never terminate on a non-converging shrinker. `DEFAULT_SHRINK_MAX_ROUNDS = 64`. |

## §6 Smoke subset chosen

Per AGENTS.md §2, the cycle changes the counterexample shrinker surface and
sandbox wire JSON shape, so the smoke subset is:

- `counterexample_tools` (11 tests, covers shrink/get/list/list-filter/
  list-pagination/events_count + m8-06's always-violating rewires ce1/ce5/ce7/
  ce10/ce11)

`e2e_connectivity` was not re-run because the change set does not touch MCP
server startup plumbing. Wall time: ~64s.

## §7 What's NOT closed by m8-06

- **m8-04 R-hypothesis-reconstruction-fidelity**: target_hypothesis persistence
  for byte-faithful replay. m9+ scope.
- **Bundle-as-blob → side table** for bundle payload columns. m9+ scope.
- **m7-07 single-call session_stop** (CLI). m9+ scope.
- **R4** (cross-variant existence predicate shrinking). m9+ scope.
- **R6 / R8** (dyn ValueTree: !Send; HypothesisInput Send+Sync+'static). m9+
  scope.

## §8 Tag and merge

- Tag: `m8-06-real-shrinkers.0`
- Merge method: `git merge --no-ff` (per project convention; the literal "FF"
  is the git fast-forward semantics applied to `9b5eed0` itself during the
  scoping cycle).
- After merge, `main` HEAD = merge commit. The branch is preserved on origin.
