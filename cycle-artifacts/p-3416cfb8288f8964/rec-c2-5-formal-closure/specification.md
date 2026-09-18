# Specification — rec-c2.5-formal-closure

## Goal

Bring the formal governance documents (`docs/ROADMAP.md`,
`reconstruction-contracts.toml`, `apply-checkpoint.json` root, vault
`cycles/index.md`) into alignment with the actual state of REC-C2:
all five sub-cycles effectively closed, with REC-C2.5 (EventBus
deletion) being the last piece to formalize.

## Non-goals

- **No Rust code change.** The repository has no remaining
  EventBus production surface. The ratchet asserts this
  (`scripts/check_legacy_evb.py` at baseline 0).
- **No new feature work.** This is a governance cycle that
  unlocks REC-C3 by retiring the REC-C2 active gate.
- **No ledger retro-sync of pre-existing cycles** beyond the
  governance reflection that REC-C2 is closed.

## Requirements

### REQ-1 — `docs/ROADMAP.md` reflects REC-C2 closure

The file must:

- Mark `REC-C2 — Legacy evidence/event-path deletion` as `CLOSED`
  in the convergence sequence (lines 36-49).
- Identify the next active gate as REC-C3 (Hexagonal boundary
  closure).
- Add a `## Active milestone` section (or update existing one)
  pointing at REC-C3 with a status of `ACTIVE` (or `NEXT` if the
  next cycle is not yet opened).
- Keep the historical section intact; only update the active
  milestone pointer.

### REQ-2 — `reconstruction-contracts.toml` reflects the gate change

The file must:

- Flip `active_gate` from `REC-C2` to `REC-C3` (or whatever the
  contract schema requires).
- Update `gates.REC-C2.status` to `CLOSED` with the latest
  sub-cycle SHA as `closed_at`.

### REQ-3 — Tag `rec-c2-5-formal-closure` is created

A git tag `rec-c2-5-formal-closure` must point at the merge commit
that lands REQ-1 and REQ-2.

### REQ-4 — SDDK cycle closes

The cycle must drive through `explore → specify → build → verify →
release → archive` with all required gate receipts in the ledger.

### REQ-5 — Root `apply-checkpoint.json` reflects the closure

The JSON must point at the new cycle, with `cycle_id:
rec-c2-5-formal-closure`, `status: closed`, `main_sha: HEAD` and
`remote_tag: rec-c2-5-formal-closure`.

### REQ-6 — Vault `cycles/index.md` row added

The vault file at
`~/.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` must contain
a new row for this cycle.

## Tier

B-direct (governance, no Rust touched). T0 only — fmt + clippy
gates cannot regress because no Rust files are touched. Branch
`check_legacy_evb.py` runs as the substantive verification.

## Out of scope

- Refreshing `reconstruction-contracts.toml` deeper than REQ-2.
  The contract schema may have additional fields that this cycle
  will not touch; if the script `check_architecture_contracts.py`
  complains about undeclared contracts, those become a follow-up
  cycle (REC-C3's opening cycle).
- REC-C2.4 explicit closure as a sub-cycle. REC-C2.4 is
  *characterization-only* by REC-C2 convention; its closure is
  recorded as a one-line note in REQ-1's history section, not as
  a new cycle.
- Resetting the ratchet inventory. `legacy-evb-inventory.json`
  reflects the current truth (baseline 0); do not regenerate.