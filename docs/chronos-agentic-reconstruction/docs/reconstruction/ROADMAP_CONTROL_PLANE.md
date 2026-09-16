# Roadmap Control Plane (convention, not an application)

Purpose: keep WIP bounded so REC-C1 + REC-C2 + M4 + sandbox + portable runtime
never advance at once again.

## Declared windows

```text
ACTIVE PRODUCT GATE  : REC-C1.5 (restart / reopen / retention / stale cursor /
                       recovery integrity) - the last REC-C1 gate where a bug can
                       make Chronos remember a run differently before and after a
                       restart. Sub-gates, no parallel gates:
                         C1.5.0 characterization      LANDED (c22732b2)
                           CHAR-RET: compaction is not durable -> same request
                             answers first_seq=0 before restart and 500 after,
                             with no gap and no watermark.
                           CHAR-REPLAY: a body-corrupt segment is SKIPPED ->
                             open() succeeds, reads start at seq 1 with ZERO gaps.
                           CONTROL: a clean restart preserves SessionId + seqs.
                         C1.5.1 retention watermark      LANDED (0be29519)
                           retained_from is the LOGICAL boundary; file deletion is
                           only physical reclamation. Crash-safe order: commit the
                           manifest atomically, activate in memory, then reclaim.
                           Boundary advances only over WHOLLY retired segments
                           (never cutoff+1). Reads below it -> PositionBeforeRetention
                           -> ServiceError::CursorStale{requested,retained_from}.
                           Reopen skips sub-watermark leftovers; a segment straddling
                           the boundary fails closed. No inference from absence
                           (RetentionMetadataMissing). RET-1..RET-10 + CONTROL green;
                           CHAR-RET flipped.
                         C1.5.2 strict replay/recovery  LANDED (3cc511ef)
                           Two-phase: build_replay_plan() validates everything, then
                           apply_replay_plan() builds a FRESH backend and swaps it in,
                           so a failure publishes nothing (no partial state). Beyond
                           the checksum, seq-space semantics are validated inside each
                           segment and continuity across segments (first live segment
                           starts at retained_from; next.start == prev.end + 1).
                           Typed ReplayIntegrityError variants via
                           LogError::ReplayIntegrity{session_id,kind}. Corruption is
                           never converted into a Gap. All three replay entrypoints
                           share the one strict primitive. C1.5.1 leftovers stay out
                           of the plan. REP-1/2/3/5/6/7/8/9/10/12 + CONTROL green; the
                           two tests documenting skip-and-continue were flipped.
                         C1.5.3 tail state (open | sealed | unclean/unknown; a
                           leftover .seg.tmp must not vanish semantically)
                         C1.5.4 reopen + registry bootstrap (rebuild the registry
                           from validated durable data, same SessionId; no
                           QueryEngine fallback; no automatic get_or_reopen per
                           read, so drop_session stays meaningful)
                       Also: ById over a truncated history must not answer "not
                       found" for an id that retention removed ->
                       EvidenceUnavailableDueToRetention.
ACTIVE RESEARCH GATE : SANDBOX-S0.2 CLOSED (frozen as experimental contract)
                       six contracts in docs/design/EXECUTION_CONTRACTS.md,
                       collect() removed from ExecutionEnvironment,
                       three-vocabulary invariants, cache CAS + namespace rename,
                       ten executable negative cases green.
                       Next research slot intentionally left EMPTY: S0.3 is not
                       opened until C1.4/C1.5 need it.

DONE
  REC-C1.0 / C1.0a characterization (5 DEF-001 locked causally)
  REC-C1.1 EventsCursorV1 (pure, opaque, typed; value-semantic advance)
  REC-C1.2 session owns ExecutionLog (typed NoExecutionLog, no backend fallback)
  REC-C1.2a canonical MANDATORY ownership: one identity, try_adopt validates it,
            session_start fails rather than degrading to EventBus-only
  REC-C1.3 traps written down (cursor bridge off-by-one, no QueryEngine/DebugTrace
            in events_read, completeness must be Unknown until C1.4)
  SANDBOX-S0.1a / S0.1a+ (runner, host+bwrap, tri-state caps, no fallback)
  SANDBOX-S0.1b / S0.1b+ (podman rootless, local pinned image, --pull=never,
                          staged inputs/outputs, verified cleanup, N1-N6)
  SANDBOX-S0.1c (qemu/kvm KERNEL class: read-only base initramfs, guest ramfs
                 overlay, serial copy-in/out, real readiness marker, no TCG
                 fallback, provenance; plus podman immutable image-id pinning)
                 -> 4 placements run ONE scenario contract; N1-N9 PASS

BLOCKED / NEXT
  REC-C1.3 events_read cutover (reads Session.ExecutionLog + EventsCursorV1,
                                per REC_C1_3_TRAPS.md)
  REC-C1.4 gap/completeness    (needs NEW tests, not recycled offset tests)
  REC-C1.5 restart/resume+retention
  REC-C2 legacy deletion       (after REC-C1.8 handoff)
  M4 adaptive instrumentation  (after REC-C2)
  SANDBOX-S0.2 scenario/result contract -> S0.3 ExecutionEnvironment port
  Portable Runtime             (after SANDBOX-S0.8 adoption review)
```

## Limits

```text
active_product_gates  = 1
active_research_gates = 1
```

Opening a third front requires closing or explicitly parking one of the two.

## Branch mapping

| Gate | Branch |
|---|---|
| REC-C1.x | `design/rec-c1-truth-cutover` (rebase onto main per cycle) |
| SANDBOX-S0.x | `spike/sandbox-s0-characterization` |
| REC-C0 history | frozen: v0.7.111, merge/release receipts, archive manifest |

## Frozen-history rule

REC-C0 artifacts are not edited after release unless demonstrable corruption.
New discoveries are recorded as REC-C1 or SANDBOX-S0 findings, even when they
originate in behavior that existed during REC-C0.
