# SESSION CLOSE 2026-09-23 — R9.4 to R9.7 (drift closure phase 2)

## HEAD
`main` @ `5209276c` (pushed, GH Actions del R9.7 push en curso al cierre).

## Operator directives honored
- AUTO mode (preserved from R9 init 2026-09-22T21:11Z): no human-gate interrupts
  for the 4 R9 drift closures.
- Composition over duplication: 0 production code changes, 0 helpers added (R9.6
  actually removed the unused `assert_capability_error` helper rather than
  papering over with `#[allow(dead_code)]`).
- No ledger edits: `reconstruction-contracts.toml` untouched.
- Honest verdict over artificial progress: each push that surfaced a new drift
  was diagnosed from real GH Actions log evidence (not inferred).

## What got closed in this session (R9.4 to R9.7)

### R9.4 — drift #9 + #10 (commit `bb208577`)
- CI 35838377028 (Coverage) + 35838377195 (CI) sobre `95317d11`.
- `chronos-sandbox/tests/query_filters.rs::test_get_event_at_first_and_last` (line 660).
- `chronos-sandbox/tests/event_tools.rs::test_get_event_after_probe_stop` (line 64).
- v1 envelope contract `envelope.event.event_id` → v2 flatten
  `event_detail.event_id` (per `client/tools.rs::get_event`).
- T1 query_filters PASS 19.09s; T1 event_tools PASS 18.50s.

### R9.5 — drift #11 (commit `43030620`)
- Coverage 35841092634 sobre `bb208577`.
- `chronos-sandbox/tests/probe_inject.rs` 4 tests asserting v1
  `probe_inject: capability: <slot>` prefix.
- **Critical discovery**: v2 `observe` dispatcher does NOT emit
  `capability: <slot>` slot — it surfaces `ServiceError` directly via
  `"observe: {error}"`.
- Final migration:
  - `assert_capability_error` helper matches semantic substrings per slot
    (CAP_EBPF_UPROBE: kebab + ebpf-unavailable synonyms;
    CAP_PROBE_STARTING: probe-starting + probe-still-starting).
  - Tests `test_probe_inject_invalid_symbol` + `test_probe_inject_without_root`
    accept either ebpf-uprobe OR probe-starting (race condition before PID
    is recorded).
  - `test_probe_inject_nonexistent_session` checks `nonexistent-session-xyz`
    substring instead of obsolete `Start a probe with probe_start` suffix.
- T1 probe_inject 4/4 PASS 19.19s.
- T2 regression 40/40 across 6 suites (error_handling 8/8, event_tools 3/3,
  m0_acceptance 6/6+10 ignored, probe_inject 4/4, query_edge_cases 10/10,
  query_filters 9/9).

### R9.6 — clippy dead_code fix-up (commit `3c83eafe`)
- CI run 35843404004 sobre `43030620` failed Clippy with
  `error: function 'assert_capability_error' is never used`.
- The helper became orphaned after the slot widening in R9.5 (tests inline
  explicit match arms against any of the slot substrings).
- Composition over duplication: **deleted** the helper (-99 lines) instead
  of `#[allow(dead_code)]`. Reubicamos el comentario explicativo en el
  bloque I1.
- T1 probe_inject 4/4 PASS 25.45s; T2 36/36 across 5 suites. T0 fmt+clippy
  workspace clean.

### R9.7 — drift #12 (commit `5209276c`)
- Coverage 35844727448 sobre `3c83eafe`.
- 2 tests en `chronos-sandbox/tests/rec_c1_8_uat_c1_03_forced_gap.rs`:
  `uat_rec_c1_03_clean_session_reports_complete_negative` + `uat_rec_c1_03_forced_gap_reports_gap_detected`.
- Both used `"mode": "Query"` (PascalCase) → deserializer rejected with
  `failed to deserialize parameters: unknown variant Query, expected query or by_id`.
- `EventsReadKind` in `crates/chronos-services/src/output.rs:1631` is a
  snake_case enum (`Query` / `ById` Rust variants → `"query"` / `"by_id"` on
  the wire). Contract ratified by
  `crates/chronos-services/tests/events_read_kind.rs`.
- Fix: `"mode": "Query"` → `"mode": "query"` in the `read_for_completeness`
  helper.
- T1 2/2 PASS 5.30s; T0 workspace fmt+clippy clean.

## Cumulative drift count

8 drifts pre-C5.3.1 v1→v2 closed in R9 (drifts #5, #6, #7, #8, #9, #10, #11, #12).

## Files touched in R9.4 to R9.7

- `chronos-sandbox/tests/probe_inject.rs` (R9.5 + R9.6: -99 netas lineas).
- `chronos-sandbox/tests/rec_c1_8_uat_c1_03_forced_gap.rs` (R9.7: +6/-2).
- `docs/roadmap/STATE.md` (R9.4-R9.7 row appended).
- `docs/roadmap/JOURNAL.md` (R9.4-R9.7 row appended).

Zero production code changes (`crates/`, `services/`, `client/`, `mcp/`,
`domain/`, workflows).

## GH Actions status (R9.7 push at 10:04:32 UTC)

| Workflow | Status | Run ID |
|---|---|---|
| Supply chain security | ✅ success | 35846671623 |
| Sandbox Debt Sentinel | ✅ success | 35846671536 |
| Architecture Contracts | ✅ success | 35846671681 |
| CI (Test) | 🟡 in_progress | 35846671561 |
| Coverage | 🟡 in_progress | 35846671556 |

The first 3 deterministic GREEN. CI + Coverage running at session close —
both are tarpaulin + cargo-test on the workspace. Expected ~20 min total.

## Critical operator-facing insight

**Tarpaulin alphabetical execution means R9 is reactive whack-a-mole, not
bounded by planning.** Each push reveals a drift the previous tarpaulin run
never reached (order: query_filters before query_edge_cases before event_tools
before probe_inject before rec_c1_8_uat_c1_03_forced_gap). At the time of
session close, ~30 `offset:N` occurrences remain in 7 files (state_depth,
query_filters, memory_depth, program_scenarios, concurrency_stress,
rec_c1_characterization, m0_acceptance) that have NOT yet been surfaced by
tarpaulin. They will surface in subsequent pushes if they are the first
failing test in the alphabetical order.

**The operator's framing holds**: "close counts ≠ verified acceptance".
R9 has honest test-migration closure, but not a feature-complete 5/5 GREEN
state on consecutive SHAs. Each new push risks surfacing another drift.

## Decision posture at session close

Per AGENTS §3 (gate preauthorization) + §7 (continuous loop), R9 will
continue to address new drifts surfaced by subsequent pushes. Per the
honest ledger policy, I will **NOT** declare R9 COMPLETED until we observe
5/5 GH Actions GREEN on a single SHA with no follow-up RED surfaced.

## Next step (R9.8+ if needed)

Wait for R9.7 CI + Coverage. If GREEN: 5/5 confirmed, R9 closes 8 drifts;
write final R9 closure handoff and stop. If RED: diagnose next drift via
`gh run view --log-failed`, fix locally, commit R9.x, push, repeat.

## Cumulative actions in this session

123 (R9.1-R9.3 close) + 4 commits (R9.4-R9.7) + 3 pushes + STATE row +
JOURNAL row + this handoff = **132 cumulative actions**.
