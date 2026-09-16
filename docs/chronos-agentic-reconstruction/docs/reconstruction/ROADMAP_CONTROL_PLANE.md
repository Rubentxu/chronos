# Roadmap Control Plane (convention, not an application)

Purpose: keep WIP bounded so REC-C1 + REC-C2 + M4 + sandbox + portable runtime
never advance at once again.

## Declared windows

```text
ACTIVE PRODUCT GATE  : REC-C1.0  (characterization baseline)  -> REC-C1.1 next
ACTIVE RESEARCH GATE : SANDBOX-S0.1a (s0run + host/bwrap)     -> S0.1b next

BLOCKED / NEXT
  REC-C1.1 cursor model        (unblocks after C1.0 evidence, done)
  REC-C1.2 session owns log    (after C1.1 gate)
  REC-C2 legacy deletion       (after REC-C1.8 handoff)
  M4 adaptive instrumentation  (after REC-C2)
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
