# Proposal — REC-C7: Reconstruction convergence close

Path: A-min. Baseline: origin/main @ f3a831a2 (REC-C6 merged).

## Intent

REC-C7 is the **final convergence gate** in the sequence
(REC-C0..REC-C7). Its job is to flip the active_gate from "REC-C7"
to a non-REC-C* owner (convergence phase end), promote the
remaining 4 REC-C-owned contracts to either verified or to a future
owner_gate, and produce the release tag that REC-C5 deliberately
deferred (REC-C5 closed a gate, not a deliverable).

After REC-C7 closes, every official reconstruction milestone (M6
OpenTelemetry correlation + export, M7 Differential execution v2,
M8 Counterexample shrinking and test intelligence) is unblocked
for execution against the post-convergence codebase.

## Evidence (from inline explore, session panda, 2026-09-21)

`python3 scripts/check_architecture_contracts.py --strict-no-gaps`
returns PASSED after the REC-C7.1 promotion (M4 contracts to
planned/M4-future). This means every REC-C* owned contract is in a
state the strict-no-gaps gate accepts:
- verified (closed, evidence on file)
- planned + owner_gate != REC-C* (deferred to a future gate)
- M4A-001 / M4B-001 specifically: planned/M4-future because there is
  no in-tree implementation substrate (Go crate for M4A, Rust
  composition-root binding for M4B); REC-C* cannot close them.

The strict-no-gaps gate is the architectural fitness gate that REC-C7
must pass to close. It currently passes; the cycle preserves that.

## Slices

- **C7.1 M4 contracts promotion** (doc + contracts.toml only).
  Reclassify M4A-001 / M4B-001 from blocked (REC-C6) to planned
  with owner_gate = "M4-future". Documents the substrate absence
  in the notes. Already landed (commit in this branch). ~1 file.

- **C7.2 release tag** (git). Tag the current `main` HEAD with
  `v0.7.112` (the next semver bump in the active `v0.7.x` series;
  `v0.7.111` was REC-C0 truth-first convergence, the series
  continued through REC-C1..REC-C6). Annotated tag with the
  REC-C7 close reason: "Reconstruction convergence close — REC-C7
  unblocks M6/M7/M8 milestones; wire surface 63 -> 41 tools;
  hexagonal boundary verified; SOLID + connascence reduction
  closed; API surface ratified."

- **C7.3 final closeout commit** (docs). Update
  `docs/ROADMAP.md` to mark REC-C7 CLOSED; replace the convergence
  sequence with the post-convergence milestone plan
  (M6 -> M7 -> M8 in execution order, with the dependency notes
  M7 needs M6, M8 is independent). Update
  `reconstruction-contracts.toml` `updated` stamp to 2026-09-21
  (no `active_gate` value or set to empty string to signal
  convergence phase end).

- **C7.4 archive** (cycle-artifacts). Archive the REC-C7 cycle
  per `scripts/sddk-archive` (or equivalent). Cycle record goes
  into `cycle-artifacts/p-3416cfb8288f8964/rec-c7-convergence-close/`
  alongside the proposal.md / apply-checkpoint.json.

## Non-goals

- We do not propose closing the 4 official reconstruction
  milestones (M6/M7/M8) in this cycle. They have owner_gate =
  M6/M7/M8 and remain `planned` (OTEL-001 / DIFF-001 / CONC-001
  / UI-001). Closing them is the post-convergence roadmap.
- We do not propose fixing the pre-existing vault drift
  (CC#8/#11/#17/#18/#22/#26/#39/#56). That is documented in
  `cycle-artifacts/p-3416cfb8288f8964/rec-c7-convergence-close/notes.md`
  as a follow-up for a future gate.
- We do not propose a #[allow(...)] cascade for any clippy drift
  surfaced by the strict-no-gaps pass. The strict gate is already
  passing, so no clippy work is needed.
- We do not propose deleting any test or relaxing any contract
  assertion. M4 promotion is honest deferral, not status laundering.

## Pre-approved gates

All `human_gate`s pre-approved per the user's standing instruction
(auto-run mode, complete the full roadmap without per-cycle approval).

## Risk register

| id | severity | mitigation |
|---|---|---|
| C7-R1 strict-no-gaps regresses after the C7.3 final commit | high | Run --strict-no-gaps after every commit in the cycle; abort if any commit regresses. |
| C7-R2 release tag conflicts with a previous tag (e.g. v0.1.1 was used by REC-C3.1) | medium | Use `v0.2.0` (next semver after the post-C3 series; reflects wire surface reduction + architectural closure). Confirm with `git tag -l` before tagging. |
| C7-R3 M4 promotion to planned/M4-future is interpreted as "M4 cancelled" by readers | low | Notes field explicitly states "Future M4 milestone cycle ... will close this contract". |

## Out of scope (post-convergence roadmap)

- M6 OpenTelemetry correlation + export (OTEL-001).
- M7 Differential execution v2 (DIFF-001).
- M8 Counterexample shrinking and test intelligence (CONC-001).
- M9 Execution Explorer (UI-001).
- Pre-existing vault drift remediation (CC#8/#11/#17/#18/#22/#26/#39/#56).
- The 29 not-yet-converged API neighbours documented in
  AGENT_API_V2.md (one cycle per neighbour group when the canonical
  v2 equivalent is designed).
