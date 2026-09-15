# Chronos roadmap

> **Status:** reconstruction convergence phase. See `docs/chronos-agentic-reconstruction/`
> for the target architecture, compliance ledger, convergence gates and official milestones.

## Active milestone

**REC-C0 — Restore the truth baseline — Status: in_progress**

The 2026-09-15 source audit found that the reconstruction delivered real foundations but still contains material contract gaps and parallel legacy architecture. Official reconstruction M6 is therefore **blocked** until REC-C0..REC-C7 close.

Immediate work:

- restore default CI/coverage to green;
- compile/test feature-only code explicitly;
- make `/reconstruction-contracts.toml` the current requirement truth ledger;
- enforce architecture dependency and legacy-use ratchets in CI;
- reconcile README/API capability claims with verified behavior;
- then cut agent-visible evidence over to the authoritative ExecutionLog.

Primary plan:

`docs/chronos-agentic-reconstruction/docs/reconstruction/CONVERGENCE_PLAN.md`

Acceptance:

`docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md`

## Convergence sequence

1. **REC-C0 — Restore the truth baseline** — ACTIVE
2. **REC-C1 — ExecutionLog cutover and truthful reads**
3. **REC-C2 — Legacy evidence/event-path deletion**
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
