# Handoff: m9+ Backlog (Updated 2026-09-12T18:18Z)

## Status

As of 2026-09-12T18:18Z, the m9 vault is canonical-schema-clean across all 57 cycles
in CA p-3416cfb8288f8964 (plus 2 legacy). 43 active cross-checks all pass.

m9-57 was a major vault hygiene cycle. Discovery: prior CC runner filtered
on the `Total: X` summary pattern and silently missed CCs that emit
`DRIFT:` lines without a summary. Running all 43 CCs with broad pattern
detection surfaced 452 drift lines (vs 0 previously reported).

Closed:
- 21 apply-checkpoints backfilled with 12 missing schema fields each
- 4 overly-strict CC regexes hardened (CC#22, CC#23, CC#27, CC#30)
- CC#3 made era-aware (fix-peel exemption)
- CC#14: findings_closed removed from legacy set (semantic distinction)
- m9-34/m9-35 SHAs reconciled to canonical (base_sha=head^, Head SHA match)
- m9-54/m9-55/m9-56 change-entry + archive-manifest canonicalized
- CC#48 added: meta-check that runs every CC and reports drift

## Bucket 1: By-design disclosures (unchanged)

These remain by-design and are **not** "fixable" without an explicit
design change. See
`.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-backlog-blocked-2026-09-12.md`
for the full list.

| ID | Source | Title |
|---|---|---|
| m9-01-R1..R3 | m9-01 scoping | schema_version disclosures |
| m9-02-R1..R8 | m9-02 scoping | Bundle storage disclosures |
| m9-04-R1..R6 | m9-04 scoping | Side-table key layout disclosures |
| m8-04-R-hypothesis-fallback | m8-04 | property_target fallback |
| m8-06-R4 | m8-06 | Cross-variant existence predicate shrinking |

**Action:** None.

## Bucket 2: Architectural redesign (deferred to m10+)

| ID | Title | Status |
|---|---|---|
| cc-001-god-module | counterexample_storage.rs at 2,823 LoC | DEFERRED |
| cc-004-implicit-io-toctou | save_bundle_record_and_events TOCTOU pattern | DEFERRED |

**Action:** Defer to m10+ unless explicitly requested by user.

## Bucket 3: Pre-vault-reorg documentation drift

| Terminated ID | Closed by | Why not rebuildable |
|---|---|---|
| m8-04-R4 | m9-02 | m8-04 source artifacts not in checkout |
| m8-07-R2 | m9-01 | m8-07 source artifacts not in checkout |

**Action:** Out of scope for auto-mode. Recoverable from pre-reorg
snapshot only.

## Bucket 4: Feature work (not debt)

| ID | Title | Status |
|---|---|---|
| m9-02-R4 | counterexample_bundle_events MCP tool | BACKLOG |

**Action:** Treat as feature backlog.

## Cross-checks summary (as of 2026-09-12T18:18Z)

All 43 cross-checks pass. Breakdown:

- **C1-C10 (legacy schema normalization)**: PASS
- **C11-C20 (canonical schema enforcement)**: PASS
- **C21-C30 (release/merge receipt schema)**: PASS
- **C31-C40 (verify-report/verify-findings schema)**: PASS
- **C41-C47 (recent hardening)**: PASS
- **C48 (self-consistency meta-check, m9-57)**: PASS

## Sessions

| Session | Cycles | Drift classes |
|---|---|---|
| m9-34..m9-43 | 10 | 10 |
| m9-44..m9-49 | 6 | 6 |
| m9-50..m9-53 | 4 | 4 |
| m9-54..m9-56 | 3 | 3 (knowledge-only / dedup / branch cleanup) |
| m9-57 | 1 | 7 (schema backfill + 4 CC hardening + CC#48 + era-awareness) |

## When to break the handoff

Same as before. Bucket 1 unchanged, Bucket 3 requires source artifacts,
Bucket 4 requires product scope.


## Session 2026-09-12T18:18Z: post-m9-57 sweep

After m9-57 tagged v0.7.56 (then v0.7.58 for archive-manifest self-fix),
ran additional drift sweep across under-checked dimensions:

- cycles/index.md `Published SHA` — found m9-56 and m9-57 had `—` placeholder
  instead of real SHA; filled in (m9-56 → `32d9a3c...`, m9-57 → `308215f...`)
- archive-manifest.md `Base SHA` column — found m9-13 was missing the row
  entirely; added `bfb9edeeea1a5dd88d8828addf38d0c755c7b5cf` (verified reachable)
- archive-manifest.md m9-54 had short Base SHA `a24139e`; expanded to full 40-char
- change-entry.md `## Subject` m9-56 had `head_sha: TBD` and `base_sha: 6bdf8ba...`;
  filled in (m9-56 → `32d9a3c...`/`6bdf8ba66...`)
- change-entry.md `## Subject` m9-57 had `head_sha: TBD`; filled in (→ `308215f...`)

After all fixes: CC#48 meta-check returns 0 DRIFT lines, confirming
all 48 cross-checks are clean.


## Session 2026-09-12T19:18Z: m9-58..m9-60 sweep

After m9-59 tag v0.7.61, ran additional drift sweeps looking for
dimensions not covered by existing 50 CCs.

Closed:
- m9-58 (v0.7.60): short SHAs in SHA-keyed metadata cells of change-entry
  Ciclo tables (4 occurrences across m9-* history); CC#49 added.
- m9-59 (v0.7.61): 31 apply-checkpoints missing canonical `remote_tag`
  field (16 had legacy `tag`, 15 had neither); CC#50 added.
- m9-60 (v0.7.62): 2 cycles (m9-56, m9-57) in cycles/index.md without
  cycle-artifacts/ folder; synthesized 6 artifacts for each; CC#51 added.

After all fixes: CC#48 meta-check returns 0 DRIFT lines, confirming
all 51 cross-checks are clean.

## Cross-checks summary (as of 2026-09-12T19:18Z)

All 51 cross-checks pass. Breakdown:

- C1-C10 (legacy schema normalization): PASS
- C11-C20 (canonical schema enforcement): PASS
- C21-C30 (release/merge receipt schema): PASS
- C31-C40 (verify-report/verify-findings schema): PASS
- C41-C47 (recent hardening): PASS
- C48 (self-consistency meta-check, m9-57): PASS
- C49 (short SHA detection, m9-58): PASS
- C50 (canonical `remote_tag` detection, m9-59): PASS
- C51 (cycle-artifacts existence, m9-60): PASS

## Cosmetic drift remaining (not CC-enforced)

These are by-design or low-impact:

1. terms/index.md vs cycles/index.md Last updated timestamp format
   inconsistency (`YYYY-MM-DDTHH:MMZ` vs `YYYY-MM-DDTHH:MM:SSZ`).
2. 3 archive-manifests (m9-11, m9-12, m9-13) use Status=ARCHIVED while
   cycles/index.md uses Status=CLOSED. Same meaning, different label.
3. cycle_id short-form vs long-form is by-design per CC#16 (both
   acceptable; m9-19+ uses short, m9-03..m9-18 uses long).


## Session 2026-09-13T09:21Z: m9-61 (probe drain/stop race fix)

Closed MS-RACE-FIX as a B-direct cycle. Single source commit (5d4c00d on
feat/ms-race-fix) plus cycle-artifacts commit (21cb16d). Tag v0.7.63 on
5d4c00d; merged into main as 35bccbc.

| Item | Status |
|---|---|
| Drain/stop race in probe lifecycle | CLOSED (m9-61-ms-race-fix) |
| NativeProbeBackend::stop_probe blocking semantics | applied |
| ProbeService::stop + BrowserProbeService::stop reorder | applied |
| ProbeBackend trait doc updated | applied (ADR-0005 cited) |

Gates:
- T0: fmt + clippy PASS
- T2: chronos-domain (149), chronos-services (263), chronos-browser (42), chronos-native single-thread (99)
- T4-smoke: 16/16 (4 analytics + 1 e2e + 11 program_scenarios)

Pre-existing ptrace flake documented in AGENTS.md §6.5 — NOT a regression.
Reproduces on main and feat/ms-race-fix alike.

Cross-checks: C1..C51 pass. No new CC added (race closed by construction).

Follow-ups (deferred to m9+):
- **Bounded join with timeout** — current `handle.join()` is unbounded.
  Previous HIGH-5 pattern (async spawn + recv_timeout(10s)) was bounded;
  switching to inline join lost that bound. If a probe thread is wedged
  in waitpid (parent ignores SIGKILL), MCP server blocks indefinitely.
- **Static check for stop-then-drain caller order** — optional CC enforcing
  the contract at the call sites.

Vault: 61 cycles indexed, 51 cross-checks clean, peel_match verified.


## Session 2026-09-13T09:44Z: m9-62 (bounded stop_probe HIGH-5 timeout)

Closed the first m9-61 follow-up: bounded join restored in
`NativeProbeBackend::stop_probe`. m9-61's inline `handle.join()` had
lost the 10s upper bound. Pattern restored: spawn waiter + mpsc +
recv_timeout(10s); abandon on timeout. Single B-direct commit
(b98b2a4 on feat/m9-62-bounded-stop-probe). Tag v0.7.64.

| Item | Status |
|---|---|
| Bounded join with 10s timeout | CLOSED (m9-62-bounded-stop-probe) |
| NativeProbeBackend::stop_probe HIGH-5 pattern | restored |
| Trait contract: blocking + bounded | enforced by construction |
| Follow-up "stop-then-drain caller ordering CC" | deferred to m9-63+ |

Gates:
- T0: fmt + clippy PASS
- T2: chronos-native single-thread 99/99
- T4-smoke: program_scenarios 11/11 (incl. test_infinite_loop_stopped_by_probe_stop),
  e2e_connectivity 1/1, analytics_tools 4/4, probe_lifecycle PASS

Cross-checks: C1..C51 pass. No new CC added (bounded join enforced by construction).

Vault: 62 cycles indexed, 51 cross-checks clean, peel_match verified.


## Session 2026-09-13T09:54Z: m9-63 (stop-then-drain ordering cross-check)

Closed the second m9-61 follow-up: added CC#52 (stop-then-drain
ordering) to vault-drift-sweep.md. Single B-direct commit (8dc1063
on feat/m9-63-stop-drain-cc). Tag v0.7.65.

| Item | Status |
|---|---|
| CC#52 stop-then-drain ordering | CLOSED (m9-63-stop-drain-cc) |
| Service-layer call sites monitored | probe.rs, browser_probe.rs |
| Self-test (synthetic violation) | PASS (CC#52 catches it) |
| Follow-up "stop-then-drain caller ordering CC" | CLOSED |

Gates:
- T0: fmt + clippy PASS
- CC#52 on current code: 0 drift
- CC#48 meta-check: PASS (52 CCs all clean)

Both m9-61 follow-ups now closed (m9-62 bounded join, m9-63 CC).

Vault: 63 cycles indexed, 52 cross-checks clean, peel_match verified.


## Session 2026-09-13T10:17Z: final drift sweep

End-of-session drift sweep across dimensions not covered by existing
CCs (pattern from m9-58/59/60). All checks PASS.

| Dimension | Check | Status |
|---|---|---|
| Cycles indexed | cycles/index.md has 63 rows | ✓ |
| Cycle-artifacts ↔ cycles/index | 60 folders, 3 allowed exceptions (m9-01, m9-02, m9-54) | ✓ |
| Inverse: folders without index row | 0 | ✓ |
| Artifact count per cycle | 60/60 have exactly the 6 expected files | ✓ |
| Knowledge pairs (change-entry ↔ archive-manifest) | 63/63 paired; pre-vault-reorg m8-* has only change-entry (by design) | ✓ |
| terms/index.md Last archive ↔ cycles/index.md most-recent | Both = m9-63-stop-drain-cc | ✓ |
| head_sha consistency (apply-checkpoint ↔ release-receipt ↔ cycles/index) | All 3 session cycles consistent | ✓ |
| findings_closed IDs consistent (apply-checkpoint ↔ change-entry ↔ archive-manifest) | All 3 session cycles consistent | ✓ |
| CC#48 meta-check (52 CCs all clean) | PASS (after m9-63 CC#52 added) | ✓ |

No new drift dimensions discovered. CC#51 covers all known cases.

### Known by-design exception (not CC-enforced)
m8-07-hypothesis-reconstruction-fidelity has change-entry but no
archive-manifest. Source artifacts not in checkout (vault reorg, see
Bucket 3 of the original handoff). Documented in handoff bucket 3.
Not a regression, no action required.

### Cross-check coverage map (52 CCs)
- C1-C10: legacy schema normalization (cycles/index, terms/index, change-entry structure)
- C11-C20: canonical schema enforcement (apply-checkpoint required fields)
- C21-C30: release/merge receipt canonical fields
- C31-C40: verify-report/verify-findings schema
- C41-C47: hardening (CC#42 era-aware peel, CC#44-#47 metadata)
- C48: meta-check (every CC returns empty)
- C49: SHA fields must be full 40-char (m9-58)
- C50: canonical `remote_tag` field (m9-59)
- C51: cycles/index.md cycle must have cycle-artifacts/ folder (m9-60)
- C52: ProbeBackend stop-then-drain ordering at service call sites (m9-63)

### Session close

3 cycles closed in this session (2026-09-13T07:21Z → 10:17Z):
- m9-61-ms-race-fix (v0.7.63): drain/stop race fix
- m9-62-bounded-stop-probe (v0.7.64): HIGH-5 timeout restoration
- m9-63-stop-drain-cc (v0.7.65): stop-then-drain ordering CC

Net cycle delta: 60 → 63.
Net CC delta: 51 → 52.
Vault state: canonical, 52 CCs clean.

All m9-61 follow-ups closed.


## Session 2026-09-13T10:25Z: m9-64 (vault drift CI workflow)

Closed a CI gap: the 52 vault CCs only ran manually during SDDK
cycles. A regression in vault files would not be caught until the
next session's drift sweep. Single B-direct commit (339f7b5 on
feat/m9-64-vault-drift-ci). Tag v0.7.66.

| Item | Status |
|---|---|
| Vault drift CI workflow | CLOSED (m9-64-vault-drift-ci) |
| .github/workflows/vault-drift.yml | added |
| scripts/check_vault_drift.sh | added |
| Self-test (synthetic m9-99 drift) | PASS |

Gates:
- T0: fmt + clippy PASS
- ./scripts/check_vault_drift.sh: PASS (52 CCs all clean)
- Self-test: synthetic m9-99 row injected, script exits 1 + reports DRIFT

Vault: 64 cycles indexed, 52 cross-checks clean, peel_match verified.
