# rec-c3-ci-hygiene — Session Handoff 2026-09-19

This handoff captures the state at session close. Resuming work requires reading
this file first; it has the SHAs, gate run IDs, pending actions, and the
discoveries that drove each slice.

## Cycle
- `cycle_id`: rec-c3-ci-hygiene
- `project_id`: p-3416cfb8288f8964
- `milestone`: REC-C3 — Hexagonal boundary closure
- `sub_cycle`: rec-c3-ci-hygiene — baseline CI / Coverage / Vault drift reconciliation
- `path`: A-min
- `branch`: rec-c3-ci-hygiene (kept around for traceability, fast-forwarded to main)
- `head_sha` (anchor, immutable per CC#12 + operator rule #8): `2e761d4f76dc4c7b2a78972753770ac2aedf0357`
- `branch_head_sha`: `d4c945c89eca3ed5beb783d855aad08cdfafd482`
- `main_sha_after_integration`: `65754dcb85d8...` (vault INTEGRATION-COMPLETE commit)
- `status`: CLOSED (preserved per CC#11)
- `operational_status`: `OPEN_REMOTE_GATE_PENDING` → will flip when main 5/5 confirmed

## Final state at session close
- Branch HEAD `d4c945c8` shipped with **5/5 GH Actions GREEN** on the rec-c3-ci-hygiene branch:
  - Vault Drift Sweep: `35473907163` ✅
  - Architecture Contracts: `35473907165` ✅
  - Sandbox Debt Sentinel: `35473907266` ✅
  - CI: `35473907171` ✅
  - Coverage: `35473907211` ✅
- Fast-forwarded rec-c3-ci-hygiene → origin/main at 2026-09-19T23:26Z (main went from `fa5eb582` to `d4c945c8`).
- Vault INTEGRATION-COMPLETE commit landed on main at `65754dcb` and triggered 5 new GH Actions runs.
- **At session close, 3/5 GREEN on main HEAD `65754dcb`:**
  - Vault Drift Sweep: `35476230662` ✅
  - Architecture Contracts: `35476230641` ✅
  - Sandbox Debt Sentinel: `35476230625` ✅
  - CI: `35476230635` 🟡 in_progress
  - Coverage: `35476230622` 🟡 in_progress
- Background watch task `149277gd5f` is watching CI+Coverage on main.

## Operator rules respected
1. ✅ 5/5 GH Actions GREEN required before ff-merge — verified at branch HEAD before merge.
2. ✅ No validator modification.
3. ✅ Full SHA chain preserved — fast-forward only, no squash/rebase/cherry-pick/merge commit on the branch.
4. ✅ No `#[ignore]`, no `--skip`, no waiver.
5. ⏳ Tren B blocked until 5/5 GREEN on integrated main HEAD — pending CI+Coverage confirmation.
6. ✅ REC-C1.5.2 strict replay not modifiable.
7. ✅ Historical SHA anchor `head_sha = 2e761d4f76dc...` immutable.
8. ✅ `status: CLOSED` preserved (CC#11); reopen state in `reopen_after_cih_d` block + `operational_status` field.
9. ✅ Stash@{0} = `f97b5798...` in quarantine (not touched).
10. ✅ Stash report at `session-handoff/STASH_REPORT_2026-09-19_cih-d.md` committed.
11. ✅ CIH-E rules: explicit session scope, real session from `probe_start`, no `--skip`, isolation between sessions.
12. ✅ CIH-F original rules: SessionRepository unchanged, no concrete adapter imports in services, REC-C1.5.2 intact, no `dyn Any`/downcasts, no second authoritative registry.
13. ✅ CIH-F review rules: peek panics on poisoned mutex (not silently Absent); concurrency discriminants prove the peek→reopen→register race is fixed; corrections are minimal inside the registry.
14. ✅ CIH-G rules: cursor assertion at line 114 untouched, no blind waits, ExecutionLog is canonical authority, probe_drain is not.
15. ✅ CIH-H rules: regenerated CC#4 SHA AFTER the last code commit that touches sessions.rs AND AFTER `cargo fmt --all`.
16. ✅ Did not flip `status=CLOSED` into certification while `operational_status=OPEN_REMOTE_GATE_PENDING`.
17. ⏳ Final report pending — see "Pending for next session" below.
18. ✅ T0/T1 + smoke real first, then push + 5 remote gates on exact cycle HEAD.

## Commits on rec-c3-ci-hygiene (chronological)
| SHA | Slice | Description |
|---|---|---|
| `8f49d48d` | CIH-F v2 | atomic rehydrate + peek-panics-on-poison + 3 concurrency discriminants |
| `59b2dfcc` | CIH-G | UAT-C2-01 root cause: bounded poll for log advance + diagnostic |
| `e471221c` | CIH-H | regenerate m9-71 archive-manifest Artifact index SHA (CC#4) — replaced by 29f0bef6 |
| `29f0bef6` | CIH-H-followup | correct sessions.rs SHA row to match post-fmt file content |
| `b98b747f` | vault | record CIH-H-followup |
| `fd9dae9e` | CIH-G-fix-2 | first drain limit=1 + cursor-based `wait_for_log_advance` |
| `437eded8` | CIH-G-fix-3 | bail on empty log + 10s deadline for tarpaulin margin |
| `d4c945c8` | vault | record CIH-G-fix-3 — branch HEAD with 5/5 GREEN |
| `65754dcb` | vault | INTEGRATION-COMPLETE note on main |

## Discoveries that drove each slice (read these to understand WHY before resuming)

### CIH-F v2 (atomic rehydrate)
The concurrency review of the SessionExecutionLogRegistry rehydrate path established by CIH-F v1 surfaced three corrections, all minimal and inside the registry:

1. **Atomic `rehydrate(dir, session_id)`** replaces the open TOCTOU race between `peek_registration`, the factory call, and `register`/`register_unavailable`. Two-pass design: peek under `logs.lock()`, drop the lock to call the factory, re-acquire and re-check so a concurrent winner is observable. Returns a typed `RehydrateOutcome` (AlreadyAvailable / AlreadyUnavailable / Registered / RecordedUnavailable).

2. **`peek_registration` no longer collapses `Err(poisoned)` into `RegistrationState::Absent`**. Old behaviour was wrong on both concurrent-correctness (a racing caller would proceed thinking the slot was empty) and failure-visibility axes (the panic was the actionable signal). Now `expect("poisoned during peek_registration")` — consistent with `try_reopen` and `register`.

3. **`#[cfg(test)] pub fn poison_for_tests()`** — deterministic poison affordance for the discriminant below.

Three concurrency discriminants pinned:
- `cih_f_peek_registration_panics_on_poisoned_lock` — uses `poison_for_tests` + `#[should_panic(expected = "...")]`
- `cih_f_concurrent_rehydrate_linearised_for_same_session` — 16 concurrent rehydrate calls on multi_thread runtime, 4 workers, Barrier sync; asserts exactly 1 Registered outcome, all 15 racers get AlreadyAvailable pointing to the SAME Arc
- `cih_f_concurrent_rehydrate_unavailable_preserves_first_reason` — 8 concurrent rehydrate against missing on-disk log; asserts exactly 1 RecordedUnavailable and all racers get AlreadyUnavailable carrying the SAME reason string

### CIH-G (UAT-C2-01 timing root cause)
Two distinct signatures surfaced under tarpaulin-instrumented CI runners:

1. **Original `tokio::time::sleep(2s)` not always enough** — replaced by `wait_for_first_event` bounded poll on `probe_drain_log` with observable exit criterion (events.length > 0). Deadline: 10s (was 5s; 5s not enough under tarpaulin — observed 5078ms first event).

2. **`cursor remains identical` on second drain** — NOT a server-side probe_drain bug; cursor staying identical is the CORRECT behaviour when the log has produced no new records between drains. The fixture's `test_busyloop` generates events at a rate that the first drain can consume in one pass. Fix: bounded poll `wait_for_log_advance` on `probe_drain` (NOT `probe_drain_log`, so we hit the same handler and read total_buffered which is the only signal that says "the log has new records" without consuming them).

**CIH-G fix-2 used wrong exit signal** — `total_buffered` on `ProbeDrainResponse` is `events.len()` of THIS page (canonical_drain.rs:299 `raw_events: events.len()`), NOT a log total. Two same-size pages both report the same count, so `total_buffered > first_total` never fires once the first drain grabs everything in one shot. **Fix-3 switched to comparing cursor STRINGS** because the canonical ECV1 token includes the seq in its last segment, so a different token means new records were examined. This is the monotonic signal that the helper actually needs.

**CIH-G fix-3 deadline bump** — CI tarpaulin on stressed runners takes 6-8x longer for the fixture to produce its first event than the fixture's claimed 3s runtime. 5s deadline is not enough margin; 10s is 2x the worst observed and still a hard cap (no blind sleep).

**First drain uses limit=1 + raw `call_tool`** — the wrapper `probe_drain_with_evidence_cursor` hardcodes limit=1000 which would consume the entire fixture output in one shot. limit=1 guarantees the first drain cannot consume everything; the drain logic pairs each requested Raw with its derived TripwireFired records (canonical_drain.rs logical-boundary check), so even limit=1 returns the Raw plus its firing records.

### CIH-H (manifest SHA regen)
The SHA-256 row for `crates/chronos-services/src/sessions.rs` in `m9-71-services-list-store-contract/archive-manifest.md` was stale because CIH-F v2 modified the file. **`scripts/regen_manifest_index_shas.py` rewrote the row, but the regen MUST run AFTER `cargo fmt --all`** (first regen at e471221c wrote SHA cfd02d4f... but the file committed in 8f49d48d was 50e4d619... — the difference is `cargo fmt --all` reformatted the rehydrate() call chain). CIH-H-followup at 29f0bef6 corrected this.

Per AGENTS.md §5: this ritual runs AFTER the last code commit that touches `sessions.rs` AND AFTER `cargo fmt --all` reformats any source file whose content a manifest row points at.

### Coverage flake exposed (rec_c1_7_projection_restart_equivalence)
**Pre-existing bug, NOT introduced by my changes.** `event_counts_by_type` in `crates/chronos-query/src/engine.rs:314` uses `sort_by_key(|a| std::cmp::Reverse(a.1))` — when two types have the same count (62/62 syscall_enter/syscall_exit), the sort leaks HashMap iteration order, which is non-deterministic between runs. rec_c1_7 hits this surface because its fixture is ptrace-traced and the assertion compares the same query response before and after a restart.

Fix is a one-line tie-break in `engine.rs:318` (`sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)))`) — but **this was NOT in the authorized slice scope (CIH-F / CIH-G / CIH-H only)**. The flake ended up not blocking because CIH-G-fix-3 changes to test timing apparently shook the HashMap iteration order to a stable pairing. If rec_c1_7 re-emerges in a future gate, the fix is one line in `engine.rs` — but it should go through its own slice (CIH-I?) with operator authorization.

## Local tier gates at session close (verified on integrated main HEAD d4c945c8)
| Tier | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| T3 (workspace lib + integration, excl. sandbox/e2e/native) | `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e --exclude chronos-native --no-fail-fast` | all test_result lines `0 failed` |
| T3 native serial | `cargo test -p chronos-native --lib -- --test-threads=1` | 108/108 passed |
| T4-smoke subset | `cargo test -p chronos-sandbox --test multi_session --test rec_c2_2_uat_c2 --test e2e_connectivity --test analytics_tools -- --test-threads=1` | 16/16 passed |
| Vault drift | `scripts/check_vault_drift.sh` | exit 0 (48 python CCs + 7 bash CCs clean) |

## Pending for next session

1. 🔲 **Wait for 5/5 GH Actions GREEN on origin/main HEAD `65754dcb`** (background task `149277gd5f`).
   - CI: `35476230635`
   - Coverage: `35476230622`
   - Run `gh run watch 35476230635 35476230622 --exit-status` from inside `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` (or `/tmp/cy-main`) — bash is in the latter.

2. 🔲 **Write the final report** with:
   - Branch SHA: `d4c945c8`
   - Main SHA: `65754dcb` (after INTEGRATION-COMPLETE commit)
   - All gate run IDs (branch + main)
   - UAT-C2-01 cause + fix summary
   - CIH-F v2 concurrency review outcome
   - Dependency ledger confirming 3 waivers unchanged and no services→ebpf or services→browser reopening
   - Tren B unblock statement per operator rule #5

3. 🔲 **If any gate RED on main**: diagnose + own to a new slice; do NOT silently re-merge.

4. 🔲 **After 5/5 GREEN on main**: Tren B (REC-C3.3.3) is officially unblocked per operator rule #5.

## Key files to touch when resuming
- `cycle-artifacts/p-3416cfb8288f8964/rec-c3-ci-hygiene/apply-checkpoint.json` — has all slice entries + remote_verification_at_d4c945c8 + INTEGRATION-COMPLETE note
- `cycle-artifacts/p-3416cfb8288f8964/rec-c3-ci-hygiene/close-receipt.md` — to be created
- `cycle-artifacts/p-3416cfb8288f8964/rec-c3-ci-hygiene/release-receipt.md` — needs update (or already exists)
- `session-handoff/STASH_REPORT_2026-09-19_cih-d.md` — already committed, don't touch

## Resume command (suggested)

```bash
# from /var/mnt/DiscoChino2-fast/Proyectos/rust/chronos or /tmp/cy-main
cd /var/mnt/DiscoChino2-fast/Proyectos/rust/chronos
gh run watch 35476230635 35476230622 --exit-status
# then write the final report
```
