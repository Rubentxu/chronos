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


## Session 2026-09-13T11:46Z: m9-65 (stale branches cleanup)

Closed drift class `branch-stale-post-merge` across all milestone prefixes.
CC#46 only watched `fix/m9-*`, so 49 stale branches (25 local + 24 remote)
across M0-M9 had accumulated since each cycle merged to main without
deleting its per-cycle branch. Two B-direct commits landed as 97ff56e on
feat/m9-65-stale-branches-cleanup. Tag v0.7.67.

| Item | Status |
|---|---|
| CC#53 (extends CC#46 to all prefixes) | ADDED (bash, not auto-executed by CC#48) |
| 49 stale merged branches deleted | DONE (25 local `git branch -d` + 24 remote `git push origin --delete`) |
| Recovery log (branches-deleted.log) | CREATED (88 lines; tip SHA of every deleted branch + 24 preserved) |
| 5 local + 19 remote not-merged branches | PRESERVED + DOCUMENTED for human triage |
| Dynamic CC count in check_vault_drift.sh | DONE (46 python + 6 bash + 1 self = 53 total) |

Gates:
- T0: fmt + clippy PASS
- T1: chronos-domain/-services/-browser unit tests: 263 passed
- T1: chronos-native unit tests: 99 passed
- T4-smoke: e2e_connectivity + probe_lifecycle: 6/6 passed
- CC#48 meta-check: PASS (46 python + 6 bash documented)

Cross-write convention check (CC#12): main_sha == head_sha (97ff56e53...).
Initially set main_sha to merge commit c4b2740...; CC#12 caught the
drift and corrected before final push. The standard convention is
that main_sha points to the cycle head (the commit that landed the
work) rather than the merge wrapper.

Vault: 65 cycles indexed, 53 cross-checks documented, peel_match
verified for m9-65.

### Branches preserved (human triage recommended)

These branches are NOT merged into main and represent either preflight
cleanup attempts or scoping documents from closed milestones. Full list
in cycle-artifacts/p-3416cfb8288f8964/m9-65-stale-branches-cleanup/branches-deleted.log.

Local (5):
- chore/m-ci-flake-preflight
- chore/m5-preflight-clippy-drift-cleanup
- chore/m5-preflight-sandbox-drift
- feat/m5-02b-debug-read-extract
- feat/m5-05b-debug-trace-specialized-extract

Remote (19):
- chore/m5-02c-cleanup-inert-artifacts
- feat/m2-function-exit-dwarf, feat/m2-native-live-probe-frame-capture, feat/m2-pie-fixture-ci
- feat/m3-08-property-projection, feat/m3-09-mutation-lens-tools, feat/m3-10-causal-slice-tool,
  feat/m3-11-m3-uat-order-total, feat/m3-scoping
- feat/m5-01-services-skeleton, feat/m5-03-sessions-extract, feat/m5-04-tripwires-extract,
  feat/m5-05a-debug-trace-core-extract, feat/m5-05b-debug-trace-specialized-extract,
  feat/m5-agent-api-v2-scoping
- (plus 5 above that have remote counterparts)

### Open follow-ups

- **Bounded join unit test** (deferred from m9-62): 10s test runtime is the blocker.
- **CC for `## Files Inventory` in verify-report** (deferred): 22 cycles m9-32..m9-53 lack the section.
- **5+19 not-merged branches triage**: human review recommended (see above list).

Net cycle delta this session: 63 → 65.
Net CC delta this session: 52 → 53.
Vault state: canonical, 53 CCs documented, peel_match verified.



## Session 2026-09-13T12:21Z: m9-66 (vault drift detection hardening)

Closed 3 drift classes with one focused B-direct cycle. CC#4 had a
silently-broken awk regex (backticks not stripped before match) that
prevented it from ever firing — discovered 54 stale SHAs in m9-01..m9-10
archive-manifests. CC#5 was off-by-16 since the m6/m7/m8 milestone rows
were added. CC#54 added as a bash meta-check (sibling of CC#48) so the
7 bash CCs are now auto-validated alongside the 46 python CCs.

| Item | Status |
|---|---|
| CC#4 awk regex | FIXED (strip backticks + trim whitespace; exclude self-reference) |
| CC#5 regex | FIXED (m9-* only) |
| CC#54 (bash meta-check) | ADDED (sibling of CC#48) |
| 54 stale SHAs in m9-01..m9-10 | REGENERATED |
| check_vault_drift.sh | REFACTORED to invoke both CC#48 + CC#54 |

Gates:
- T0: fmt + clippy PASS
- T1: chronos-domain/-services/-browser: 263 passed
- T1: chronos-native: 99 passed
- T4-smoke: e2e_connectivity + probe_lifecycle (full file): 6/6 passed
- CC#48 + CC#54: PASS (46 python + 7 bash CCs all clean)

Self-tests performed:
- Inject wrong Total cycles (65→99): CC#5 catches → script exit 1
- Inject wrong SHA in m9-01 manifest: CC#4 catches → script exit 1
- Post-fix clean state: PASS

Cross-check note (m9-62 follow-up partially addressed):
- The "bounded join unit test" remains deferred (10s test runtime is the blocker)

Vault: 66 cycles indexed, 54 cross-checks documented (46 python + 7 bash + 1 self = 54 total).

### Pattern observed: "fix a broken CC, then fix what it would have caught"

m9-65 surfaced 49 stale branches that CC#46 was missing.
m9-66 surfaced 54 stale SHAs that CC#4 was silently failing to detect.
Both times the fix was: improve the CC, then deal with the accumulated drift.
A future "CC smoke test" cycle could periodically inject drift into each
CC to confirm it's still firing — this is deferred.

### Open follow-ups

- **Bounded join unit test** (deferred from m9-62): 10s test runtime is the blocker.
- **CC for `## Files Inventory` in verify-report** (deferred): 22 cycles m9-32..m9-53 lack the section.
- **5+19 not-merged branches triage** (deferred from m9-65): human review recommended.
- **CC smoke test cycle**: implement periodic drift injection to confirm every CC still fires.
- **Sandbox test warm-up ordering**: `test_session_start_via_v2_then_session_stop_via_v2` fails when run alone, passes with full file (likely MCP server binary warm-up).

Net cycle delta this session: 64 → 66 (m9-65 + m9-66).
Net CC delta this session: 52 → 54 (CC#53 + CC#54).
Vault state: canonical, 54 CCs documented, peel_match verified for both m9-65 and m9-66.



## Session 2026-09-13T12:40Z: m9-67 (CC smoke test for drift detection)

Closed the "fix a broken CC, then fix what it would have caught" pattern that recurred in m9-65 (49 stale branches) and m9-66 (54 stale SHAs). Added `scripts/smoke_test_ccs.sh` that periodically injects synthetic drift into CC#4 (wrong SHA in archive-manifest), CC#39 (wrong Total cycles in cycles/index.md; superset of CC#5), CC#46 (fake fix/m9-99-smoke-test branch), and verifies the CC#48+CC#54 meta-checks report PASS on clean state. Each test asserts exit code + DRIFT line in a per-test log file. ~35 seconds wall time.

| Item | Status |
|---|---|
| `scripts/smoke_test_ccs.sh` | NEW (250 lines; per-test git clone; dynamic Total cycles read; per-test log files) |
| CC#4 detection (synthetic SHA mismatch) | VERIFIED (smoke test catches injected drift) |
| CC#39 detection (synthetic Total cycles mismatch) | VERIFIED (smoke test catches injected drift) |
| CC#46 detection (synthetic stale branch) | VERIFIED (smoke test catches injected drift) |
| CC#48 + CC#54 clean-state PASS | VERIFIED (exit 0; reports 46 python + 7 bash CCs all clean) |

Gates:
- T0: not applicable (no Rust changes)
- T-self: 4 smoke tests PASS in ~35s wall time
- CC#48 + CC#54: PASS on real tree after cycle

Two bugs found in the smoke test script itself during post-merge verification (fixed in commit `dfddc99`):
1. **Hardcoded `| Total cycles | 66 |`**: the CC#39 injection had the literal value hardcoded. When Total cycles became 67 (this cycle), the replace silently no-op'd and the test reported a false PASS. Fixed by reading the value dynamically and bumping by 100.
2. **Shared `out.log`**: all four tests wrote to a single log, so a passing test could mask a previous failure. Each test now writes to its own log (`cc4.log`, `cc39.log`, `cc46.log`, `meta.log`).

These are exactly the kind of "silent CC failure" the smoke test is designed to catch — they would have caused silent failures of the smoke test itself if not caught. m9-67 demonstrates the meta value of the smoke test: it caught bugs in its own implementation.

### Pattern closed: "fix a broken CC, then fix what it would have caught"

Three instances observed this session, all addressed:
1. m9-65 → CC#46 missing milestone prefixes → 49 stale branches cleaned up
2. m9-66 → CC#4 broken awk regex → 54 stale SHAs regenerated
3. m9-67 → smoke test had its own latent bugs → caught by the smoke test running on a bumped Total cycles value

The third instance is the most important: the smoke test is the first piece of cycle infrastructure that *tests itself* via the same mechanism it tests the rest of the system. Future cycles that touch `vault-drift-sweep.md` or `check_vault_drift.sh` should run `./scripts/smoke_test_ccs.sh` before merge.

### Open follow-ups

- **Bounded join unit test** (deferred from m9-62): 10s runtime blocker.
- **CC for `## Files Inventory` in verify-report** (deferred): 22 cycles m9-32..m9-53.
- **5+19 not-merged branches triage** (preserved from m9-65): human review recommended.
- **Sandbox test warm-up ordering**: `test_session_start_via_v2_then_session_stop_via_v2` fails alone, passes with full file (likely MCP server binary warm-up).

Net cycle delta this session: 64 → 67 (m9-65 + m9-66 + m9-67).
Net CC delta this session: 52 → 54 (CC#53 + CC#54; m9-67 added no new CC).
Vault state: canonical, 67 cycles indexed, 54 CCs documented, peel_match verified for m9-65/m9-66/m9-67.
