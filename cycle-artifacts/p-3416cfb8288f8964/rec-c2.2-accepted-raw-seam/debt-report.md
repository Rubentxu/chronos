# Technical Debt Report: rec-c2.2-accepted-raw-seam

> **Generated from `debt-report.json`, which is the authoritative artifact.**
> This Markdown is a projection; on any disagreement, the JSON wins.

## Subject

| Base | Head | Diff digest |
|---|---|---|
| `e4fd938c` | `1619fb25` | `a182963b1086eba8…` |

## Decision

**Verdict: `PASS_WITH_WARNINGS`** (fail_closed=True, re_iterate_from=none)

- **no-introduced-critical-or-high** — Zero introduced findings at severity HIGH or CRITICAL; the single MEDIUM finding is attribution:pre_existing (owned by FIND-C2.2-04), so Decision Contract rows for FAIL do not apply.
- **low-band-introduced-findings** — Seven unsuppressed introduced LOW findings with no blocker: below the PASS_WITH_WARNINGS triggering threshold row, so the terminal row 'No warning or blocking condition' would yield PASS; the gate reports PASS_WITH_WARNINGS to keep the duplication (FIND-DEBT-000003) and dead-trait-method (FIND-DEBT-000001) follow-ups attached to the cycle record rather than silently aging.
- **pre-existing-visible-owned** — The only MEDIUM finding is pre-existing and owned by tasks.md FIND-C2.2-04 with a recorded next-cycle plan; per policy it stays visible and non-blocking. It is not emitted as an INC file because its durable record already exists as a named, owned task finding in the cycle's tasks.md.

## Coverage

Required clusters: coupling, duplication, overeng, smells

Completed: coupling, duplication, overeng, smells · Failed: none

## Summary

- total: 8
- by severity: {"critical": 0, "high": 0, "medium": 1, "low": 7}
- by attribution: {"introduced": 7, "pre_existing": 1, "unknown": 0}
- by cluster: {"coupling": 2, "duplication": 2, "overeng": 2, "smells": 2}

## Findings

| # | Severity | Priority | Attribution | Cluster | Finding |
|---|---|---|---|---|---|
| 1 | low | P1 | None | coupling | ProbeBackend::read_since has no production caller after the switchover; its contract varies by backend |
| 2 | low | P1 | None | coupling | ProbeService::drain derives the scan budget from offset+limit, coupling wire pagination to evidence examination |
| 3 | low | P2 | None | duplication | Fail-closed decode + typed error idiom repeated in four log readers (two added by this cycle) |
| 4 | low | P2 | None | overeng | ProbeService::drain dependency set (SessionExecutionLog, EventsCursorV1, canonical_drain, resolver pipeline, closure) judged proportionate |
| 5 | low | P2 | None | duplication | canonical_drain.rs sibling readers share record-scanning shape but differ at every decision point; extraction judged a shallow abstraction |
| 6 | low | P3 | None | smells | legacy_cursor wire field is written only as null and asserted only as none |
| 7 | medium | P2 | None | smells | BrowserAdapter::take_semantic_events remains a destructive read, relocated off-trait and exempted by FIND-C2.2-04 |
| 8 | low | P2 | None | overeng | EbpfAdapter::read_since destructive-fallback removal is enforced by construction, not by a running test |

## Follow-up

| Priority | Owner | Action |
|---|---|---|
| P2 | unassigned | Extract decode_raw_fail_closed helper; adopt in all four readers (canonical_drain.rs x2, events_log_read.rs x2, projection.rs). |
| P1 | unassigned | Decide ProbeBackend::read_since's fate: remove from trait or split trait so non-peekable backends do not implement refusals. |
| P1 | unassigned | Serve offset from the examined page or deprecate offset windows in favor of evidence_cursor continuation; document max_raw_events semantics. |
| P3 | unassigned | Drop legacy_cursor wire field at next wire revision or document an expiry. |
| P2 | unassigned | Own under FIND-C2.2-04: wire browser capture through the accepted-Raw seam; retire take_semantic_events. |
| P3 | unassigned | Optional: source-level ratchet asserting no ProbeBackend impl in chronos-ebpf calls drain_events(); else accept by-construction guarantee. |

Waivers: none

