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

## Ratchet plan

Each subphase flips its test to the corrected assertion and removes the matching
`DEF-001` item from `reconstruction-contracts.toml`:

| Subphase | Expected DEF-001 count after | Tests turned green |
|---|---|---|
| C1.0 (now) | 5 | none (baseline locked) |
| C1.3 events_read cutover | 2 | CHAR-1, CHAR-3 (offset honoured) |
| C1.4 gap/completeness | 1 | CHAR-2, CHAR-4 (empty beyond tail) |
| C1.5 restart/resume + retention | 0 | CHAR-5 (tail termination) |

Counts are provisional; the principle is that every DEF-001 entry must be
retired by a subphase that demonstrates the specific behavior it names.
