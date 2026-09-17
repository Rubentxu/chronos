# Release Receipt — REC-C1.6

**Merge SHA**: 83ee38e2 (main)
**Base SHA**: 5bbf7748 (main)
**Tag**: rec-c1-6-lifecycle-retention-wire
**Branch**: feat/rec-c1.6-lifecycle-retention-wire
**Closed at**: 2026-09-17

## Branch → main

`git merge --no-ff feat/rec-c1.6-lifecycle-retention-wire` landed on
`main` as commit `83ee38e2`. Tag `rec-c1-6-lifecycle-retention-wire`
points at the merge commit. Both `main` and the feature branch were
pushed to `origin`; the tag was pushed with `--tags`.

## Commit timeline (since base)

| SHA | Title |
|---|---|
| `0af8c3c9` | docs(rec-c1.6): reconcile C1.5 closure + C1.6 active in roadmap |
| `75492dd6` | docs(rec-c1.6): refine design — connected_sessions is the liveness source |
| `dccabee5` | chore(rec-c1.6): proposal + design + tasks + apply-checkpoint |
| `7197c40e` | feat(rec-c1.6): refuse delete_session on a live session (A1+A2+A3) |
| `7afb47ad` | feat(rec-c1.6): wire liveness marker for typed delete_session refusal |
| `1da26f5c` | feat(rec-c1.6): RetentionFacts + TailFacts on LogReadPage (B1+B2+B3) |
| `580a8f45` | feat(rec-c1.6): wire retention/tail into events_read success + CursorStale (B4+B5+B6) |
| `d1891ed3` | style(rec-c1.6): fmt + clippy -D warnings clean (C1) |
| `fbd0242d` | chore(rec-c1.6): apply-checkpoint + verify-findings + receipt + cycles index (C3+C5) |
| `83ee38e2` | Merge branch 'feat/rec-c1.6-lifecycle-retention-wire' |

## Surface impact (recap)

- **delete_session**: typed refusal on a still-live probe via
  ServiceError::SessionStillActive; no auto-stop; the marker is wired
  at session_start{action=spawn|attach} and cleared on session_stop.
- **events_read success**: gains `retention` and `tail` top-level keys.
- **events_read CursorStale**: carries TWO content items (text + json).
- **No MCP tool signature changes.**

## Out of scope respected

- No auto-stop on delete (per design constraint).
- No retention policy wire field invented (facts ≠ policy).
- REC-C2 still BLOCKED until REC-C1.8 handoff.

## Vault drift status

- CC#11: PASS
- CC#39: KNOWN PRE-EXISTING DRIFT, NOT INTRODUCED BY THIS CYCLE (CC#39
  only counts m9-* dirs; rec-c1-* cycles bump cycles/index.md Total
  cycles but CC#39 does not see them). Documented in apply-checkpoint
  gates block. M1+ follow-up.

## Archive status

Archive phase (sddk-archive) is the next step: write the cycle's
delta-spec into the long-term vault. Status: `pending` until that
phase runs.
