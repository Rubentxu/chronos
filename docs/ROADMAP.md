# Chronos roadmap

> **Status:** reconstruction convergence phase. See `docs/chronos-agentic-reconstruction/`
> for the target architecture, compliance ledger, convergence gates and official milestones.

## Active milestone

**REC-C1.6 — Lifecycle-safe delete + retention/tail facts on the wire — Status: ACTIVE**

C1.5 (RESTART/RECOVERY) closed at `5bbf7748` on `main`. C1.6 closes
the two gaps C1.5 explicitly deferred:

1. `delete_session` must refuse to delete a still-live probe with a
   typed error (no silent probe stop, no partial state tear-down).
2. `events_read` must surface retention and tail facts on the success
   path (`retained_from_seq`, `history_truncated`, `tail.state`,
   `tail.tail_seq`), and `CursorStale` must carry structured
   `requested_next_seq` + `retained_from_seq` instead of being parsed
   out of error text.

C1.6 does **not** start REC-C2 (legacy/event-path deletion); that
remains blocked behind the REC-C1.8 handoff. C1.6 also does **not**
invent a "retention policy" name on the wire — only facts already in
the manifest become visible.

C1.6.0 (first commit of the cycle) reconciles the documentary drift
left by C1.5 close (`active_gate` in reconstruction-contracts.toml +
the "Active milestone" line above). The first cycle commit may touch
both the contracts file and the roadmap to put them on the same side.

Primary plan: `cycle-artifacts/p-3416cfb8288f8964/rec-c1-6-lifecycle-retention-wire/proposal.md`
Acceptance: REC-C1's remaining acceptance gates (C1-01 10k / two
consumers / forced gap, C1-05 time semantics) exercised end-to-end
against the production wire after C1.6 ships.

## Convergence sequence

1. **REC-C0 — Restore the truth baseline** — CLOSED (C0.5-D sentinel)
2. **REC-C1 — ExecutionLog cutover and truthful reads** — IN PROGRESS
    * REC-C1.0..C1.4 truth invariants, REC-C1.5 restart/recovery — CLOSED
    * REC-C1.6 lifecycle-safe delete + retention on wire — **ACTIVE**
    * REC-C1.7 (placeholder until C1.6 plans land)
    * REC-C1.8 handoff (gate before C2)
3. **REC-C2 — Legacy evidence/event-path deletion** — BLOCKED until REC-C1.8
4. **REC-C3 — Hexagonal boundary closure**
5. **REC-C4 — SOLID + connascence reduction**
6. **REC-C5 — Canonical Agent API convergence**
7. **REC-C6 — Close unfinished M1–M4 reconstruction contracts**
8. **REC-C7 — Reconstruction convergence close**

Only REC-C7 can unblock official reconstruction **M6 OpenTelemetry correlation + export**.

## Closed historical delivery cycles

The following cycle/milestone records remain historically closed. Their close state does **not** automatically mean that every reconstruction requirement is currently verified; residual obligations are tracked in `reconstruction-contracts.toml` and owned by REC-C gates.

- **m0-truth-first-foundation** — closed
- **m5-agent-api-v2** — closed 2026-09-10
- **m6-v2-spec-surface-reduction** — closed 2026-09-11
  - internal API sub-cycle; distinct from official reconstruction M6.
- **m7-v2-spec-introspection** — closed 2026-09-11
  - internal API sub-cycle; distinct from official reconstruction M7.
- **m8-counterexample-shrinking** — closed 2026-09-11
  - foundation delivered; official roadmap M8 still requires end-to-end test-intelligence completion.
- **m9-vault-hygiene** — closed 2026-09-14
  - repository governance/vault hygiene; distinct from official reconstruction M9.
- **m10-vault-ms-cleanup** — closed 2026-09-15
  - repository governance namespace; distinct from official reconstruction M10 Execution Explorer.
- **rec-c1-5-closure** — closed 2026-09-17 (merged --no-ff into `main` as `a1a79c80`; tag `rec-c1.5-closure`).
  - canonical ExecutionLog root resolver, MCP startup bootstrap, durable delete, durable seal on clean stop. Four real-process sandbox UATs (R1..R4) plus the readiness invariant.

## Naming rule

From this point forward:

- `REC-C*` = reconstruction convergence gates;
- `M*` = official product reconstruction milestones;
- governance/vault/housekeeping cycles must use a non-product prefix and must not be presented as completion of an official `M*` milestone.

This removes the previous ambiguity where an internal `m10-*` cycle could be confused with M10 Execution Explorer.

## Specification truth

Machine-readable current compliance:

`/reconstruction-contracts.toml`

Architecture/spec fitness gate:

```bash
python3 scripts/check_architecture_contracts.py
```

REC-C7 strict close:

```bash
python3 scripts/check_architecture_contracts.py --strict-no-gaps
cargo check --workspace --all-targets --all-features
cargo test --workspace -- --test-threads=1
```

## Official future milestones after convergence

1. **M6 — OpenTelemetry correlation + export**
2. **M7 — Differential execution v2**
3. **M8 — Counterexample shrinking and test intelligence**
4. **M9 — Concurrency intelligence / happens-before**
5. **M10 — Execution Explorer**
6. **M11 — Additional language depth**

See `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` for detailed scope and gates.

## Cycle serialization lock

Only one product/convergence milestone is `Status: in_progress` at a time. The owning cycle must complete, be explicitly blocked, or be abandoned before another product/convergence milestone is promoted. Repository housekeeping may run independently only when it cannot alter product-delivery claims or acceptance evidence.
