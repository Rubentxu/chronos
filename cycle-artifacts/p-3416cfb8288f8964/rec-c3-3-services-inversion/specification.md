# REC-C3.3 specification — C3.3.0 recon + dependency map

**Cycle:** `p-3416cfb8288f8964/rec-c3-3-services-inversion`
**Path:** A-min
**Phase:** Specify
**Updated:** 2026-09-18
**Scope:** recon + dependency map (C3.3.0 only). No code changes.

## Why this is a recon-only cycle

C3.3 is the third sub-cycle of REC-C3 (Hexagonal boundary closure).
The C3.1 cycle declared the ports. The C3.2 cycle hardened the gate.
C3.3 does the **services-side inversion**: chronos-services consumes
the ports, not concrete adapters; this closes `HEX-002` (currently
`gap`) and dissolves the three carry-over findings `C31-DEBT-01/02/03`.

The C3.3 inversion is the largest single piece of architectural work
in REC-C3 (four edges across two crates). Before moving any types,
the operator asked for a recon pass that:

1. inventories the edges exactly,
2. names the use cases that justify each edge,
3. proposes where the composition root should live.

This cycle is **C3.3.0 only**. The exploration-report.md delivers all
three. C3.3.1..5 are **separate cycles**, each one an ADR or a code
sub-cycle with its own spec/design/tasks.

## Sub-cycle roadmap (informational; not in this cycle)

```text
REC-C3.3 services-side port inversion
├── C3.3.0  recon + dependency map                     <-- THIS cycle
├── C3.3.1  identity/storage seam (ADR + moves)
│          * ExecutionLogProvider placeholder → real
│          * SessionId type divergence → single owner
├── C3.3.2  composition root (chronos-services::composition)
├── C3.3.3  migrate use cases (services → ports)
│          * list_sessions / save_session / load_session
│          * explain / export / diff / output
│          * counterexample bundle port
│          * probe / browser controllers via ports
├── C3.3.4  mechanical gates
│          * HEX-C32-03 services-half: services → NotificationSink
│            (never HttpWebhookSink), and services → ports::ProbeController
│            (never NativeProbeBackend), services → ports::SessionRepository
│            (never chronos_store::SessionStore), services → ports::EbpfInjector
│            (never chronos_ebpf::EbpfAdapter), services → ports::BrowserController
│            (never chronos_browser::BrowserAdapter).
├── C3.3.5  ratchet / closure
│          * each edge removed from known_dependency_violations
│          * HEX-002 gap → verified
```

C3.3.1..5 are NOT started by this cycle. The operator gates each.

## Architectural rule (frozen at the top of C3.3)

```text
chronos-services
      ↓
chronos-domain::ports
      ↑
composition root
      ↓
native / ebpf / browser / store / webhook
```

`chronos-services` MUST NOT import `chronos_{store,native,ebpf,browser}`
directly after C3.3.3 (production code). Test-only imports in
`ce_services_tests.rs` and friends are part of C3.3.3 too.

The composition root is the **only** place where infra crates are
constructed; the only place where concrete adapters are passed into
the wired application graph. C3.3.2 places this in
`chronos-services::composition` (option 3 in exploration-report §
"Composition root decision"). The recommendation is provisional until
the ADR lands in C3.3.1.

## Requirements (this cycle: 0 new; carrying forward)

C3.3.0 introduces **no new functional requirements**. The cycle is
recon, not feature work. Requirements that motivate C3.3.1..5 already
exist in `reconstruction-contracts.toml`:

- **HEX-002** (`gap`): "Application services depend on ports, not
  concrete tracing/storage adapters." — closure criterion for C3.3.3.
- **HEX-C32-03** (`verified` for adapter→port half; services-half is
  the C3.3 mechanical rule): "chronos-webhook consumes NotificationSink
  port, not the reverse." The same pattern extends to native/ebpf/browser/store.

### Carry-over findings (re-observed, not closed)

- **C31-DEBT-01**: `ExecutionLogProvider` is a placeholder; closes when
  C3.3.1 lands the real `Arc<dyn ExecutionLogProvider>` consumer.
- **C31-DEBT-02**: `chronos_domain::session_id::SessionId` vs
  `chronos_log::SessionId` type divergence; closes when C3.3.1 decides
  ownership (ADR).
- **C31-DEBT-03**: ports/* not consumed by chronos-services; closes
  when C3.3.3 finishes the inversion.

## Out-of-scope (explicit, per operator instruction)

- `chronos-store → chronos-native` (C3.4 territory).
- Closing any carry-over finding in this cycle.
- Any code change. C3.3.0 is read-only.
- Starting C3.3.1..5.

## Acceptance criteria for THIS cycle

1. `exploration-report.md` exists at
   `cycle-artifacts/p-3416cfb8288f8964/rec-c3-3-services-inversion/exploration-report.md`.
2. The report enumerates:
   - all four `chronos-services → chronos-{store,native,ebpf,browser}` edges,
   - the files and symbols on each edge,
   - the use cases that justify each edge,
   - the composition-root candidate placements,
   - the C3.3.x sub-cycle roadmap,
   - the carry-forward findings (re-observed, not closed).
3. No file outside the recon artifacts is touched. No Cargo.toml
   changes. No source code changes.
4. `cargo check -p chronos-services --all-targets` still green (no
   regression introduced by recon).

## Verification recipe (this cycle)

- `python3 scripts/check_hex_boundary.py` — still 0 errors / 0 notes
  (recon did not touch code).
- `python3 scripts/check_architecture_contracts.py --strict-legacy` —
  still PASSED.
- `cargo check -p chronos-services --all-targets` — 0 errors / 0 warnings
  (sanity; recon is read-only).
- `cargo fmt --all -- --check` — clean.
- Manual review: the four edges match the exploration report's
  enumeration. No new edges were introduced (none could be — no code
  touched).
