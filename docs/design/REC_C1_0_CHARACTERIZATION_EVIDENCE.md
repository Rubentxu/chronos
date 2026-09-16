# REC-C1.0 — Characterization Baseline Evidence

Tests: `chronos-sandbox/tests/rec_c1_characterization.rs` (5, `#[ignore]` by default).
Baseline: main `bcee2448` (post-REC-C0 v0.7.111). Binary rebuilt from this commit.

## Command

```bash
export CHRONOS_MCP_PATH=<target>/debug/chronos-mcp
cargo test -p chronos-sandbox --test rec_c1_characterization -- --ignored --test-threads=1 --nocapture
```

Result: `5 passed; 0 failed` — the tests pass *because* they assert the current
(wrong) behavior. This is the DEF-001 baseline made executable.

## Observed behavior (2026-09-16, fixture test_busyloop)

| Test | Observation | Contract violated |
|---|---|---|
| CHAR-1 offset_pagination | total=1671, page1=10, page2(offset=10)=10, **overlap=10** | offset does not move the read position (TRUTH-002) |
| CHAR-2 offset_beyond_total | total=1679, offset=1_000_000 returned **10** events | must return empty |
| CHAR-3 limit_exact_pagination | limit=7: page1=7, page2=7, **boundary_duplicates=7** | limit/offset do not partition the log |
| CHAR-4 offset_beyond_total (edge surface) | total=1643, offset=500_000 returned **10** | must return empty |
| CHAR-5 pagination_tail_termination | total=1661, fetched=**6000** in 60 pages (walker capped) | walker never terminates at tail |

Conclusion: `query_events` ignores `offset` entirely and always returns the first
`limit` events; the tail is never reached. Both DEF-001 suites are the same bug
expressed twice.

## Ratchet rule (evidence-driven, no pre-assigned counts)

Do NOT pre-assign how many DEF-001 items each subphase retires. All five are
variants of one pagination/offset defect and C1.3 may well retire all five at
once. After each subphase:

```text
run the exact declared DEF-001 test set
        |
  a declared failure that now passes => stale waiver
        |
  remove it from reconstruction-contracts.toml immediately
        |
  remaining count = observed reality
```

C1.4 (gap/completeness) and C1.5 (restart/resume + retention) require NEW
tests for those concepts; they must not be gated on offset tests kept alive
artificially.

## Scope boundary (added in C1.0a hardening)

These tests characterize the LEGACY `query_events` surface. The target contract
is `events_read` with `EventsCursorV1 { schema_version, session_id, next_seq }`
(C1.1). No architecture is invested in promoting `offset` to the new truth.

## Causal assertions (C1.0a)

Each test now pins the cause instead of a symptom:

- CHAR-1: `page(offset=0) == page(offset=10)` exactly (not "some overlap").
- CHAR-2/4: `page(offset=1e6|5e5) == page(offset=0)` exactly (not `!is_empty()`).
- CHAR-3: all 7 returned ids duplicate AND `p1 == p2` (not an `||` that a correct
  paginator could also satisfy).
- CHAR-5: the walk stops only at its own page cap with every page full.
