# Roadmap Control Plane (convention, not an application)

Purpose: keep WIP bounded so REC-C1 + REC-C2 + M4 + sandbox + portable runtime
never advance at once again.

## Declared windows

```text
STATUS               : REC-C1.5 and REC-C1.6 CLOSED on main (tags
                       rec-c1-5-closure @ 5bbf7748..a1a79c80;
                       rec-c1-6-lifecycle-retention-wire @ 83ee38e2).
                       REC-C1.6 wired lifecycle-safe delete_session
                       (ServiceError::SessionStillActive on a live probe)
                       and surfaced retention/tail facts on the events_read
                       wire (RetentionFacts, TailFacts, structured CursorStale).
                       Two TRUTH-001 gaps remain partial: (a) the in-memory
                       QueryEngine map is still built from drain_raw_events
                       rather than from the durable ExecutionLog; (b) the
                       public UATs UAT-REC-C1-01 (two consumers, real wire)
                       and UAT-REC-C1-05 (time semantics) have not yet been
                       exercised end-to-end. Both are addressed by REC-C1.7.

ACTIVE PRODUCT GATE  : REC-C1.7 (Projection Authority + final REC-C1
                       behavioral acceptance) - the last gate that touches
                       projection plumbing before the C1.8 handoff freezes
                       C1 receipts and unblocks REC-C2.
                         C1.7.0 reconciliation                    ACTIVE
                           docs/ROADMAP + reconstruction-contracts.toml
                           bumped to REC-C1.7; cycles/index.md row added
                           (Total cycles 100 -> 101; pre-existing CC#39
                           drift grows from 1 to 2, intentionally surfaced).
                         C1.7.1 dual-truth characterization      PLANNED (RED)
                           Two non-implementing tests that prove
                           execution_query / state_query / trace_slice
                           currently diverge from SessionExecutionLog when
                           the engine map was built from a drain that
                           missed records appended afterwards.
                         C1.7.2 chronos_services::projection      PLANNED
                           Single canonical build_engine(&log). Reuses
                           events_log_read::decode (JSON-TraceEvent).
                           5 unit tests: empty / full / truncated /
                           decode-failure / noisy-filter.
                         C1.7.3 wire projection into MCP + gate    PLANNED
                           build_and_store_engine takes &SessionExecutionLog
                           (was: Vec<TraceEvent>). execution_query /
                           state_query / trace_slice call
                           require_full_history and return
                           EvidenceUnavailableDueToRetention on
                           Truncated projections.
                         C1.7.4 restart-equivalence UAT           PLANNED
                           1,000 records -> capture execution_query /
                           state_query / trace_slice -> drop engine map,
                           reopen log, re-run, assert semantic equality.
                         C1.7.5 UAT-REC-C1-01 public, real wire    PLANNED
                           10,000 records, two independent consumers via
                           events_read; producer advance; pause/resume;
                           forced gap returns GapDetected, never Complete.
                         C1.7.6 UAT-REC-C1-05 time semantics      PLANNED
                           seq/event_id/timestamp_ns deliberately
                           uncorrelated (40, 90, 130 / 10_000_500,
                           25_320_700, 25_999_001). No encoder/projection
                           substitutes one for another.
                         C1.7.7 TRUTH-001 ratchet                 PLANNED
                           partial -> verified with UAT and verify
                           command; EventBus / fired_buffer /
                           drain_raw_events / TripwireFired remain
                           as legacy paths owned by REC-C2 (not deleted
                           in this cycle).

ACTIVE RESEARCH GATE : SANDBOX-S0.2 CLOSED (frozen as experimental contract)
                       six contracts in docs/design/EXECUTION_CONTRACTS.md,
                       collect() removed from ExecutionEnvironment,
                       three-vocabulary invariants, cache CAS + namespace rename,
                       ten executable negative cases green.
                       Next research slot intentionally left EMPTY: S0.3 is not
                       opened until C1.7/C1.8 need it.

DONE
  REC-C1.0 / C1.0a characterization (5 DEF-001 locked causally)
  REC-C1.1 EventsCursorV1 (pure, opaque, typed; value-semantic advance)
  REC-C1.2 session owns ExecutionLog (typed NoExecutionLog, no backend fallback)
  REC-C1.2a canonical MANDATORY ownership: one identity, try_adopt validates it,
            session_start fails rather than degrading to EventBus-only
  REC-C1.3 events_read cutover (EventsCursorV1 + LogReadPage + cursor bridge)
  REC-C1.3a events_read completeness invariants (completeness is Unknown until
            gap evidence proves otherwise; CHAR-GAP-1..3, CHAR-COMPL-1..4)
  REC-C1.4 retention watermark + gap truth + CursorStale typed refusal
            (CHAR-GAP-RETENTION-1 flips; CHAR-COMPL-CUTOFF-1 flips;
             CHAR-COMPL-RETENTION-1 flips; GAP-COMPL green)
  REC-C1.5 retention/restart/durable delete/seal (BOOT-1..13, RESTART-1..3,
            SEAL-1..3, BYID-RETENTION green)
  REC-C1.6 lifecycle-safe delete + retention/tail facts on wire (T0..T4-smoke
            green; 11/11 in T4-smoke including lifecycle_delete, wire_retention_facts,
            restart_uat, e2e_connectivity)
  SANDBOX-S0.1a / S0.1a+ (runner, host+bwrap, tri-state caps, no fallback)
  SANDBOX-S0.1b / S0.1b+ (podman rootless, local pinned image, --pull=never,
                          staged inputs/outputs, verified cleanup, N1-N6)
  SANDBOX-S0.1c (qemu/kvm KERNEL class: read-only base initramfs, guest ramfs
                 overlay, serial copy-in/out, real readiness marker, no TCG
                 fallback, provenance; plus podman immutable image-id pinning)
                 -> 4 placements run ONE scenario contract; N1-N9 PASS

BLOCKED / NEXT
  REC-C1.8 handoff (freeze C1 receipts, classify remaining EventBus paths as
                   LEGACY owned by REC-C2, declare REC-C1 CLOSED + REC-C2
                   ACTIVE; not used to discover a new architecture)
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
