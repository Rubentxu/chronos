# Session handoff — 2026-09-18 (post-cycle close)

## Cycles touched today

1. **`retire-stale-bus-doc-mentions`** — retroactive ledger sync (A-min, 10 gates). CLOSED pre-session at `ab863cf1`.
2. **`rec-c2-5-formal-closure`** — REC-C2 governance closure (A-min, 11 gates). Tag `rec-c2-5-formal-closure` at `21ab4f24`.
3. **`rec-c3-1-application-ports`** — REC-C3.1 application ports (A-full, 12 gates including a waived UAT). Tag `v0.1.1` at `33b4f790`. **FULLY CLOSED this session**.

## Current state

- `main` @ `e4373453` (after post-release housekeeping commit), pushed.
- `release = v0.1.1` → peels to `33b4f790` (the receipts commit). Two further commits (`cb30db39`, `e4373453`) sit on top — both are post-release housekeeping; tag seal is intact.
- Vault `~/.sddk-knowledge/p-3416cfb8288f8964/`:
  - `cycles/index.md` updated with the rec-c3.1 row.
  - `changes/archive/rec-c3-1-application-ports/archive-manifest.md` written (6811 bytes).
  - INC `~/.sddk-knowledge/sddk-framework/incs/INC-RELEASE-APPLY-PERMISSIONS.md` written — **framework-side finding, ownership tracked separately, MUST NOT be lifted into a chronos cycle**.
- Ledger chain intact (344 events). Ratchet baseline still 0.
- All 164 chronos-domain tests + 614 caller tests passing; `scripts/check_hex_boundary.py` PASSES; architecture `--strict-legacy` PASSES.

## Hard invariants (chronos)

| ID | Status |
|---|---|
| HEX-001 (chronos_domain ports-* outbound purity) | `partial` (was `gap`) |
| HEX-002 (services depend on ports, not concrete adapters) | `gap` — REC-C3.3 |
| HEX-003 (storage does not depend on native tracer) | `gap` — pending |

## Debt carried forward

| ID | Owner gate | Severity | Priority |
|---|---|---|---|
| `C31-DEBT-01` ExecutionLogProvider placeholder (Cargo cyclic dep) | REC-C3.3 | low | P3 |
| `C31-DEBT-02` session_id type divergence (domain vs log) | REC-C3.3 | low | P3 |
| `C31-DEBT-03` ports not yet consumed by services | REC-C3.3 | low | P2 |
| `SDDK-GOV-RELEASE-APPLY-PERMISSIONS` framework gap | framework | low | P3 |

## Next cycle — REC-C3.2 (NOT started; awaiting operator greenlight)

**Scope**: Extract webhook HTTP transport out of `chronos_domain` into `chronos_webhook` as a proper hexagonal adapter.

**Mechanical invariants (anchor from first commit)**:

```
HEX-C32-01  chronos-domain MUST NOT depend on HTTP/webhook adapter crates
HEX-C32-02  domain source MUST NOT import HTTP client / URL / retry infrastructure
HEX-C32-03  services/application may depend on NotificationSink,
            not on a concrete webhook implementation
```

**UAT (substitutability proof)**:

```
same application use case
  + InMemoryNotificationSink  -> passes
  + WebhookNotificationSink   -> passes

domain/application semantics identical
```

**Out of scope for C3.2** (deferred to C3.3):

- C31-DEBT-01..03 (services-inversion leg of HEX-002).

**Out of scope generally**: any unrelated chronos refactor.

## Resume command

```bash
sddk cycle resume --cycle p-3416cfb8288f8964/rec-c3-1-application-ports \
  --root /var/mnt/DiscoChino2-fast/Proyectos/rust/chronos --scope .
```

(Cycle already CLOSED, so `resume` is informational here. For C3.2 the operator opens a new cycle.)

## Red flags to keep in mind for C3.2

1. **Permission registry**: `sddk release apply` requires `permissions.yaml`. Don't synthesize one to satisfy the orchestrator — use the same `git tag -a` + receipts + `transition --gate-receipt` pattern. The framework-side fix is in `INC-RELEASE-APPLY-PERMISSIONS.md`.
2. **Sequential commits on `main`**: chronos discipline is one branch, single trunk. C3.2 commits land directly on `main`, no `feat/*` side branches.
3. **`Cargo.toml` cycle/annoyance**: the `0.1.1` workspace bump is already on `main`. C3.2 should bump to `0.1.2` (patch) and tag `v0.1.2` only after reconcile of release-receipt.
4. **No scope creep**: if a "while we're here" temptation surfaces during C3.2 build, route the extra work to a separate cycle. Today's C3.1 budget was 8 commits; C3.2 should land in ≤ 8 as well.
5. **Architecture --strict-legacy before release**: must pass after build, before pushing tag. The bottleneck in C3.1 was the same — keep it green.

## Misc notes

- `~/.local/share/sddk/framework/current/` = `v1.169.75`.
- `sddk backlog item bl-bl-01M2SYSH2G000385KXH3SREAR0` carries the SDDK-GOV finding as a registered backlog item, triaged p3.
- `sddk-knowledge/p-3416cfb8288f8964/incs/` would be the natural place for Chronos-side INCs; today chronos has none (gap on the cycle ledger is closed by `apply-checkpoint.carry_forward_findings` + `debt-report.json`).
