# Exploration — rec-c2.5-formal-closure

## Why this cycle exists

REC-C2 ("Legacy evidence/event-path deletion") is the convergence gate
between REC-C1 (closed) and REC-C3 (Hexagonal boundary closure, next
gate). The roadmap says REC-C2 is "BLOCKED until REC-C1.8". REC-C1.8
closed on 2026-09-17 at `451a29b6` (tag
`rec-c1-8-authoritative-evidence-handoff`). The blocker is no longer
in effect.

REC-C2 has 5 sub-cycles:

| Sub-cycle | Title | State |
|---|---|---|
| REC-C2.0 | EventBus legacy inventory + ratchet | CLOSED at `4c7df70e` |
| REC-C2.1 | TripwireFired as ExecutionLog evidence | CLOSED at `686a364c` |
| REC-C2.2 | Accepted-Raw seam + producer convergence | CLOSED at `f02ab311` |
| REC-C2.3 | EventBus removal | CLOSED at `d435557e` |
| REC-C2.4 | Remove dual-write truth | **DEFERRED** — characterization only |
| REC-C2.5 | EventBus deletion or quarantine | **DONE in practice, not in docs** |

This cycle (`rec-c2-5-formal-closure`) formalizes the REC-C2.5
sub-cycle and confirms REC-C2.4 has effectively no remaining work.

## What landed in the wild for REC-C2.5

| Surface | Decision | Evidence |
|---|---|---|
| `chronos-domain::bus` module | DELETED | `af41d8c2` (-630 lines) |
| `ProbeBackend::read_since` | DELETED | `a99b3a35` |
| `bus_capacity` / `bus_fill` | DELETED from wire | `fb170dcc` |
| `legacy-evb-inventory.json` | baseline = 0 | `ad28430f` |
| `check_legacy_evb.py` ratchet | PASS at baseline 0 | 2026-09-17 |

The EventBus is gone in production. The "or quarantine" branch was
not taken; deletion was cleaner because no production consumer needed
the in-memory transport.

## What remains for REC-C2.4 (dual-write)

`grep -rn "dual"` over `crates/` returns only:

- **Characterization tests** (in `chronos-native/tests/`) that
  reproduce the historical dual-write shape. Per REC-C2 convention,
  these stay (archaeological trail).
- **Doc-comments** in `chronos-native/src/probe_backend.rs` and
  `chronos-mcp/src/server.rs` referencing "dual-write" as a
  historical pointer (e.g. "the dual-write shape that REC-C2
  retired"). These stay (rationale preservation).

There is **no remaining production dual-write surface**. REC-C2.4 is
effectively closed; the ratchet (`check_legacy_evb.py`) is the
authoritative assertion.

## Out of scope

- **REC-C3 — Hexagonal boundary closure** (A-full, 5 sub-cycles
  C3.1..C3.5). The next gate. Not opened here.
- **Branch cleanup beyond REC-C2/C1.6**: `feat/m5-02b-*`,
  `feat/m5-05b-*`, `design/rec-c1-truth-cutover`, spikes — these
  may be in flight outside this cycle's scope.
- **Recurring retro-sync of additional REC-* cycles** into the
  ledger. The only retroactively synced cycles in the ledger so
  far are `rec-c0-convergence-truth-gate` and
  `retire-stale-bus-doc-mentions`. REC-C2.5 will be the third.

## What this cycle does

1. Update `docs/ROADMAP.md` to mark REC-C2 as CLOSED (with the
   active gate pointing at REC-C3).
2. Update `reconstruction-contracts.toml` to reflect the new
   `active_gate`.
3. Commit + tag `rec-c2-5-formal-closure`.
4. Drive the SDDK cycle to closure in the ledger.
5. Update `apply-checkpoint.json` root.
6. Refresh vault `cycles/index.md`.

No Rust code change. Pure governance + documentation.