# Roadmap Control Plane (convention, not an application)

Purpose: keep WIP bounded so REC-C1 + REC-C2 + M4 + sandbox + portable runtime
never advance at once again.

## Declared windows

```text
ACTIVE PRODUCT GATE  : REC-C1.3 (events_read cutover)
                       core landed and hardened: stateless EventSeq read page,
                       gap-aware position, cursor rules, multi-chunk scan
                       (SCAN_CHUNK no longer leaks into page semantics),
                       fail-closed decode (no cursor emitted past unreadable
                       evidence), uniform position vocabulary. PENDING: wire the
                       events_read tool / deprecated shim onto it (drop
                       QueryEngine, DebugTraceService, CursorDto, hardcoded
                       completeness) and re-run the 5 DEF-001.
ACTIVE RESEARCH GATE : SANDBOX-S0.2 (execution contract)
                       part 1 landed: content-addressed QEMU cache with
                       validated manifest; namespace renamed to
                       chronos.execution.*. PENDING: scenario/result/capability/
                       workspace/provenance/lifecycle contract set and removal
                       of collect() from the ExecutionEnvironment draft.

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
