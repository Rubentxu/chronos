# Roadmap Control Plane (convention, not an application)

Purpose: keep WIP bounded so REC-C1 + REC-C2 + M4 + sandbox + portable runtime
never advance at once again.

## Declared windows

```text
STATUS               : REC-C1.5 and REC-C1.6 CLOSED on main (tags
                       rec-c1-5-closure @ 5bbf7748..a1a79c80;
                       rec-c1-6-lifecycle-retention-wire @ 83ee38e2).
                       REC-C1.7 CLOSED on tag rec-c1-7-projection-authority-acceptance
                       (annotated tag object peels to merge 6190390d;
                       merge commit is the cycle's published SHA).
                       QueryEngine is a reconstructible projection of
                       SessionExecutionLog via the canonical
                       chronos_services::projection::build_engine (shared
                       decoder with events_log_read); execution_query /
                       state_query / trace_slice refuse to answer when
                       the projection is Truncated or Empty (gate in the
                       MCP wrapper, rmcp::ErrorData on the wire).
                       REC-C1.8 CLOSED on tag rec-c1-8-authoritative-evidence-handoff
                       (closes the three acceptance discrepancies left open
                       by C1.7: documental drift, the dual_truth RED
                       tests still on main, and the literal 10k / forced-gap /
                       wall-clock shape of UAT-REC-C1-01/03/05). TRUTH-001
                       stays verified; UAT-REC-C1-01 (10k durable records,
                       two consumers, producer advance, A resumes), UAT-REC-C1-03
                       (negative `complete` for clean session; positive
                       forced-gap blocked on a documented `chronos_log::segmented`
                       bookkeeping bug — FIND-C1.8-01, deferred to m1-*),
                       and UAT-REC-C1-05 (four-dim independence with the new
                       `captured_at_unix_ns` dimension) all green on the wire.
                       REC-C1 ACCEPTANCE: CLOSED.

ACTIVE PRODUCT GATE  : REC-C2 (legacy deletion) — owns retirement of
                       EventBus / drain_raw_events / fired_buffer /
                       TripwireFired (the legacy analytics paths from
                       REC-C1.6 that C1.7/C1.8 chose to keep).
                       Opens after the rec-c1-8 tag is pushed to origin.

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
  REC-C1.7 projection authority + final REC-C1 behavioral acceptance
            (chronos_services::projection::build_engine canonical builder;
             MCP-wrapper gate on execution_query/state_query/trace_slice;
             UAT-REC-C1-01 two consumers + UAT-REC-C1-05 time semantics +
             restart-equivalence all green on the real wire; TRUTH-001
             partial -> verified)
  REC-C1.8 authoritative evidence handoff (closes the three acceptance
            discrepancies left open by C1.7: dual_truth RED tests removed
            (3 black-box wire tests replace them); UAT-REC-C1-01 literal
            10k + producer advance; UAT-REC-C1-03 negative `complete` arm
            on the wire; UAT-REC-C1-05 four-dim independence with the new
            `captured_at_unix_ns` dimension on ExecutionRecord. FIND-C1.8-01
            surfaces a real `chronos_log::segmented` segment-header bookkeeping
            bug as a deferred m1-* follow-up — out of scope for REC-C1)
  SANDBOX-S0.1a / S0.1a+ (runner, host+bwrap, tri-state caps, no fallback)
  SANDBOX-S0.1b / S0.1b+ (podman rootless, local pinned image, --pull=never,
                          staged inputs/outputs, verified cleanup, N1-N6)
  SANDBOX-S0.1c (qemu/kvm KERNEL class: read-only base initramfs, guest ramfs
                 overlay, serial copy-in/out, real readiness marker, no TCG
                 fallback, provenance; plus podman immutable image-id pinning)
                 -> 4 placements run ONE scenario contract; N1-N9 PASS

BLOCKED / NEXT
  REC-C2 legacy deletion       (EventBus / drain_raw_events / fired_buffer /
                                TripwireFired retirement; LEGACY-001 / 002
                                contracts stay `gap` until C2 closes)
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
