# Archive Manifest: rec-c2.2-accepted-raw-seam

## Chain

```text
verify-report.md + debt-report.json
        |
        v
release-receipt: annotated tag rec-c2.2-accepted-raw-seam -> f02ab311
        |
        v
archive-manifest: this file
```

| Item | Value |
|---|---|
| cycle_id | `rec-c2.2-accepted-raw-seam` |
| path | A-lite |
| status | **CLOSED** |
| verify | PASS (`1619fb25`) |
| debt-verify | PASS_WITH_WARNINGS (`1619fb25`) |
| release-receipt | tag `rec-c2.2-accepted-raw-seam` -> `f02ab311` |
| merge | fast-forward, `main == origin/main == f02ab311` |
| closed_at | 2026-09-17 |

## Durable knowledge

Archive is **logical closure**, per this stream's convention:
`archive_status = "ready"`, and no
`.sddk-knowledge/changes/archive/rec-c2.2-accepted-raw-seam/` directory is
created. `rec-c2.0` and `rec-c2.1` closed the same way.

## Findings carried out of this cycle

| Finding | Severity | Owner |
|---|---|---|
| FIND-C2.2-02 | info | matcher-semantics cycle: `FunctionName` no longer falls back to `SemanticEvent.description` at the accepted-Raw seam, so a syscall no longer matches a function-name tripwire. A narrowing, not a regression; the tripwire view agrees with `probe_drain` now. |
| FIND-C2.2-03 | info | a future reader that can prove loss inside the examined range: `partial` stays in the vocabulary, deliberately unproduced. |
| FIND-C2.2-04 | medium | browser-evidence cycle: browser sessions have no `ExecutionLog`, so browser captures have no durable home and the adapter buffer is lossy at capacity. |
| FIND-C2.2-05 | info | closed in-cycle: `m0_04` had been passing vacuously for three commits. |
| FIND-C2.2-06 | low | closed in-cycle: `EbpfAdapter::read_since` evicted inside a read path. Enforced by construction; runtime verification needs a BPF-capable host. |
| FIND-DEBT-000001 (P1) | low | unassigned: decide `ProbeBackend::read_since`'s fate (remove from the trait, or split so non-peekable backends do not implement refusals). |
| FIND-DEBT-000002 (P1) | low | unassigned: `ProbeService::drain` derives the scan budget from `offset + limit`, so offset-paging clients re-examine the prefix per page. |
| FIND-DEBT-000003 (P2) | low | unassigned: extract `decode_raw_fail_closed`; the fail-closed decode idiom is now at five sites across three readers. |

## Pre-existing drift, not masked

Measured, not asserted. `bash scripts/check_vault_drift.sh` on `main` before the
closure edits versus after:

```text
before: CC#11 (1), CC#12 (1), CC#18 (2), CC#19 (6), CC#39 (1)   -> 5 CCs drifting
after:  CC#18 (2), CC#19 (6), CC#39 (1)                          -> 3 CCs drifting
```

The closure **repaired** CC#11 and CC#12 (stale `cycles/index.md` SHA rows, which
`scripts/regen_manifest_index_shas.py` fixed: 18 rows across 101 manifests) and
introduced no new drift. The three that remain are all pre-existing and none of
them touch this cycle:

- CC#18: `rec-c1-7` has no `verify-findings.json` (gov-* owns).
- CC#19: 6 lines, pre-existing and unchanged by this cycle. Not enumerated in
  the `rec-c2.1` closure note either, so it is inherited rather than introduced
  by this stream.
- CC#39: index `Total cycles` vs the m9-only archive directory count
  (structural, pre-existing).

## Orchestrator dissent recorded

The debt gate classes the browser finding as `pre_existing`. The substance
pre-exists, but the relocation is introduced here: that destructive read used to
be reachable through the shared `ProbeBackend` trait and is now an inherent,
browser-local method. Recorded in `apply-checkpoint.json` rather than silently
adopted.
