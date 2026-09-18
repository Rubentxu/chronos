# Session handoff — Tren A merge + rec-c3-ci-hygiene open (2026-09-18 PM)

> Continuation of `SESSION_HANDOFF_2026-09-18.md` (morning session, retro
> ledger sync + 5 closed cycles). This document covers the PM session
> that closed Tren A (REC-C3.3.2) and opened the rec-c3-ci-hygiene cycle.
> Both files survive `git pull`; the session-scoped copy lives at
> `~/.jcode/scratch/handoff-2026-09-18-tren-a.md` for tomorrow's boot.

## Final remote state (authoritative — verify on every boot)

```bash
git fetch origin
echo "HEAD                    = $(git rev-parse HEAD)"
echo "origin/main             = $(git rev-parse origin/main)"
echo "origin/rec-c3-ci-hygiene = $(git rev-parse origin/rec-c3-ci-hygiene)"
```

> All three refs **must converge** at session boot. If they do not,
> the previous session left the repo in an inconsistent state — STOP
> and reconcile before driving Slice A. Most common cause: a handoff
> commit was authored on one branch and not fast-forwarded into main
> before session close (this exact pattern bit us earlier today).

At the time this handoff commit was authored, the three refs converged
at `c11de1fa`. A subsequent handoff-correction commit advanced them to
`5b221c98`. The **invariant** ("all three refs equal") matters; the
specific SHA does not — re-run the command above to learn the current
value.

| Field                | Value |
|----------------------|-------|
| `rec-c3-ci-hygiene` cycle status | OPEN |
| Tren B (REC-C3.3.3)  | **BLOCKED** until hygiene reaches all five gates GREEN |
| Next slice           | **CIH-A — m1_02 reconciliation with REC-C1.5.2 strict replay** |
| Working tree         | clean (only `?? .sddk-state/` session-local untracked) |

## Where we ended (pre-handoff state, for context)

- `origin/main == 16bed4d8` (after vault housekeeping commits)
- `head_sha` documented in Tren A vault: `25420e5dab54a02e46da20ecc0bacf050d2ac741`
- `merge_sha` of Tren A: `d30d025071d9f464a6ba9ff6cc50d68ce300f167`
- `released_baseline`: `147c10195f2da43b20b99b6d2d77f1c17c124242` (origin/main pre-Tren-A)
- Branch open: `rec-c3-ci-hygiene` (clean, base = `16bed4d8`, no commits yet)
- Session ID: `session_mushroom_1789711983740_fbb346f57a937d7d`

## Tomorrow's pre-flight (corrected)

```bash
# Run the verification command from "Final remote state" first.
# Confirm all three refs converge before continuing.
git fetch origin
git checkout main
git pull --ff-only origin main                 # expect same SHA as origin/rec-c3-ci-hygiene
git checkout rec-c3-ci-hygiene                 # clean, no new commits yet
# inspect cycle-artifacts/p-3416cfb8288f8964/rec-c3-ci-hygiene/apply-checkpoint.json
# inspect cycle-artifacts/p-3416cfb8288f8964/rec-c3-3-2-composition-integration-inversion/close-receipt.md
```

Then drive **Slice A (m1_02 reconciliation) directly** — no broad explore.
Evidence is already gathered: REC-C1.5.2 (commit `3cc511ef`) is the
authority; the test + comments are stale relative to that design.
Acceptance for Slice A:

```
corrupted retained segment
        ↓
open/replay
        ↓
ReplayIntegrity::CorruptSegment
        ↓
NO partially published log
NO invented Gap
NO silent salvage
```

Slice A scope is small. Slice B (Coverage / `McpTestClient::start()` under
tarpaulin) is the interesting diagnostic one; only start it after A
closes.

## Headline result

**Tren A (REC-C3.3.2) merged to `main` via fast-forward** (no squash, no
rebase, no merge commit — all 46 SHAs preserved). PR #31 closed at
`2026-09-18T22:29:15Z`. Pre-merge gates were Architecture Contracts +
Sandbox Debt Sentinel GREEN; CI / Coverage / Vault Drift Sweep were RED
exclusively by baseline pre-existing failures already reproducible on
`147c10195` (the released baseline).

Operator decision 2026-09-18 PM was **option 1**: integrate Tren A with
documented bypass of the three pre-existing RED jobs. Both pre-existing
finding IDs (`FIND-Tren-A-01` for vault drift, `FIND-CI-PRE-EXISTING-TEST-FAILURES`
for `m1_02` + `tripwire_depth`) were tagged `no_action_in_current_cycle`
(NOT permanent waiver) and handed off to `rec-c3-ci-hygiene`.

**`rec-c3-ci-hygiene` is the next cycle.** Tren B (REC-C3.3.3) is
**explicitly blocked** until hygiene closes with all five gates GREEN.

## Post-merge invariants (verified on `origin/main == 16bed4d8` before handoff commit)

> These invariants were verified at `16bed4d8`. After publishing this
> handoff, `origin/main` advanced to `c11de1fa`, but the invariants
> themselves are stable (vault-only commit, no production code touched).
> Re-verifying at `c11de1fa` should yield identical results; the table
> is authoritative.

| Invariant | Status |
|---|---|
| `services → ebpf = 0` (no waiver) | OK — 0 occurrences in `reconstruction-contracts.toml` |
| `services → browser = 0` (no waiver) | OK — 0 occurrences |
| Ledger = exactly 3 waivers | OK — `services→store`, `services→native`, `store→native` |
| `cargo fmt --all -- --check` | OK (exit 0) |
| `cargo clippy --workspace --all-targets --features chronos-services/test-utils -- -D warnings` | OK (exit 0) |
| `python3 scripts/check_architecture_contracts.py` | PASSED |
| `python3 scripts/check_legacy_evb.py` | PASSED (ratchet = 0, baseline 0) |
| `cargo test -p chronos-services --test c33_25_capability_bundle_ratchet` | 7/7 pass |

## What Tren A actually shipped (47 commits, ordered by intent)

### Real Tren A code (6 commits)

1. `7a6062b0` `refactor(domain,log): single SessionId owner; domain owns identity`
2. `9d751759` `refactor(domain,log): lift EventSeq to domain (single owner)`
3. `0e9aa473` `refactor(domain,log,services): lift ExecutionKind/ExecutionPayload/TripwireFiredEvidence to domain (single owner)`
4. `4d165a93` `refactor(domain,log): lift NewExecutionRecord/Gap/GapReason/TailState/SealedTail/ExecutionRecord to domain (single owner)`
5. `3bd71bdf` `feat(domain): define ExecutionLogProvider port (object-safe, session-scoped)`
6. `6bc5d412` `feat(log): session-scoped ExecutionLogProvider adapters + literal LogPage mapping`
7. `5ca9aeb6` `feat(c3.3.1): record_gap is canonical-evidence, port lifted`
8. `794732fa` `feat(c3.3.1): SessionExecutionLog consumes Arc<dyn ExecutionLogProvider>`
9. `93a80a5c` `docs(c3.3.1): governance — apply-checkpoint, debt-ledger, ratchets, handoff`
10. `eb258efd` `feat(c3.3.2.1a): chronos-mcp::composition module — bootstrap-scoped store wiring`
11. `764675bb` `feat(domain): ExecutionLogFactory port + ExecutionLogError::Open`
12. `d22e756d` `feat(log): SegmentedExecutionLogFactory implements the factory port`
13. `6a1c6b30` `feat(services): registry owns factory; create/reopen route through port`
14. `9f3550dc` `refactor(services): migrate call sites to registry+factory or test_support`
15. `3f46c18b` `feat(mcp): composition root builds the ExecutionLogFactory`
16. `a3d64967` `refactor(chronos-native): REC-C3.3.2 port-typed NativeProbeBackend`
17. `e48f40cb` `refactor(chronos-services): retire C33-DEBT-NATIVE-LOG-BRIDGE-01`
18. `533d5aa2` `refactor(chronos-mcp): daemon compaction reads through wrapper`
19. `1609e3c4` `test(chronos-native): integration tests port-typed`
20. `a1c76735` `test(chronos-sandbox): integration tests attach_execution_log(provider)`
21. `1fe0141d` `feat(domain,ebpf): add UprobeInjector port and EbpfUprobeInjector implementation`
22. `354e25d2` `feat(mcp): add default_uprobe_injector composition-root factory`
23. `cab65fec` `feat(services,mcp): rewire probe_inject and probe_stop through UprobeInjector port`
24. `2517cd1c` `chore(waiver): delete chronos-services->chronos-ebpf from known_dependency_violations`
25. `22ff9ca2` `feat(domain): add BrowserProbeFactory port + BrowserProbe variant in Capability`
26. `87bef0d0` `feat(browser): add BrowserProbeFactoryImpl and BrowserProbeBackend impl`
27. `d720dacd` `feat(services): migrate browser_probe service through BrowserProbeFactory port`
28. `9d108f86` `feat(mcp,browser): wire BrowserProbeFactory through composition root and server`
29. `222c3a83` `chore(waiver): delete chronos-services->chronos-browser from known_dependency_violations`
30. `a42a90cc` `docs(services): drop broken intra-doc link to BrowserAdapter`
31. `cf88fede` `feat(domain+log): split ExecutionLog into evidence + retention + maintenance ports` ← **the core Tren A split**
32. `b22a3625` `feat(services): migrate SessionExecutionLog to capability bundle (C3.3.2.5.3+5.4)`
33. `fd571666` `feat(tests): anti-stale-waiver ratchet for capability bundle (C3.3.2.6)`
34. `4ff8af72` `chore(checker): drop stale ExecutionLogProviderShape/NoopExecutionLogProvider from REC-C3.3 surface`
35. `f4ad63de` `style(fmt): apply rustfmt --edition 2021 to files touched by Tren A`

### Fixes introduced during CI iteration (4 commits)

36. `b30ab074` `test(native): align exhausted assertion with stateless page semantics` — fixes
    `m1_03_execution_log_migration::accept_and_publish_lands_in_attached_provider`. Test
    assumed `LogPage::exhausted = "tail reached after this page"`; actual semantics
    (documented in `crates/chronos-log/src/cursor.rs:39-41`) is "nothing at or after
    the input position". Fixed by reading once + reading again from `position_after`.
    This was a test bug introduced in `1609e3c4` (Tren A lateral test).
37. `5d3c9302` `fix(ci): derive_test_buckets writes zero-byte skip file when empty` — fixes
    pre-existing infrastructure bug in `scripts/derive_test_buckets.py`. When the ledger
    has no deferred failures, the script wrote a newline-terminated empty file and the
    CI awk pipeline produced a dangling `--skip ` flag with a missing argument
    (`error: Argument to option 'skip' missing`). Now writes a zero-byte file. Ratchet
    test at `scripts/tests/test_derive_test_buckets.py` (3 tests, zero deps). Tagged
    `FIND-INFRA-DERIVE-SKIP-ARGS-EMPTY` (closed in Tren A).
38. `d30d0250` `chore(vault): close C3.3.2 with merge decision and rec-c3-ci-hygiene handoff` —
    vault housekeeping after deciding option 1.
39. `5b7b7de1` `docs(vault): C3.3.2 close receipt with merge state and rec-c3-ci-hygiene handoff` —
    `close-receipt.md` artifact.
40. `16bed4d8` `chore(vault): open rec-c3-ci-hygiene cycle (skeleton apply-checkpoint)` —
    `rec-c3-ci-hygiene/apply-checkpoint.json` skeleton.

(Cycle artifact files — `apply-checkpoint.json`, `verify-findings.json`,
`close-receipt.md` — added across these 40 commits.)

### Ledger progression

```
origin/main (147c10195) = 5 waivers
HEAD post-Tren-A        = 3 waivers (services→store, services→native, store→native)
post-Tren-B (C3.3.3)    = 1 waiver  (store→native)
post-REC-C3.4           = 0 waivers
```

The `services→ebpf` and `services→browser` waivers were deleted in
`2517cd1c` and `222c3a83` respectively (commit messages confirm deletion
from `known_dependency_violations`).

## Pre-existing failures NOT introduced by Tren A (verified reproducible)

### A. `m1_02_execution_log_persistence_impl`

`chronos-sandbox/tests/m1_acceptance.rs:231` test case 6 truncates the
first of two segments by 8 bytes to corrupt its BLAKE3 checksum, then
expects `SegmentedExecutionLog::open(...)` to succeed with
`tail_seq() == Some(EventSeq::new(2))`. Comment says "First segment is
skipped; second segment still recovers."

**The code does NOT do that.** `crates/chronos-log/src/segmented.rs::apply_plan`
(REC-C1.5.2 commit `3cc511ef`) is atomic / fail-closed: any corrupt
segment → `ReplayIntegrity { CorruptSegment { ... } }`. No implicit
salvage. No invented `Gap`. This is deliberate design per REC-C1.5.2
("corruption ⇒ typed error, no invented evidence").

Test was written by `b1233b64` (2026-09-08, m1-02) describing what the
author wanted the code to do, but the code was deliberately changed
later. The test + comment are stale; the production semantics are
correct.

Repro:
```bash
git checkout 147c10195
cargo test -p chronos-sandbox --test m1_acceptance m1_02_execution_log_persistence_impl
# exit 1, panic: ReplayIntegrity { CorruptSegment { ... } }
```

### B. `tripwire_depth` (6 tests)

`chronos-sandbox/tests/tripwire_depth.rs` — all 6 tests fail because
`McpTestClient::start()` (line 17:10 etc.) expects to launch the MCP
server as a subprocess. Tests pass under `cargo test` on dev machines
because the binary is built, but fail under the tarpaulin coverage
harness because it can't find / build / launch the subprocess inside
its instrumentation environment.

The tests are not broken; the harness integration is.

### C. Vault drift CC#18/19/39

16 lines of pre-existing drift on `origin/main`, 0 introduced by Tren A:
- CC#18 (9 lines): 9 cycles in `origin/main` without `verify-findings.json`
- CC#19 (6 lines): free-text notes in `rec-c2.1` / `rec-c2.2`
- CC#39 (1 line): index count mismatch

`scripts/check_vault_drift.sh` exits non-zero on these.

## How Tren A was integrated (pre-handoff transcript)

```bash
git fetch origin
git checkout main
git pull --ff-only origin main                  # main @ 147c10195
git merge --ff-only origin/rec-c3.3-train-a     # Fast-forward to d30d0250
git push origin main                            # origin/main @ d30d0250
git commit -am "docs(vault): C3.3.2 close receipt ..."
git push origin main                            # origin/main @ 5b7b7de1
git commit -am "chore(vault): open rec-c3-ci-hygiene cycle ..."
git push origin main                            # origin/main @ 16bed4d8
git checkout -b rec-c3-ci-hygiene
git push -u origin rec-c3-ci-hygiene            # branch open, no commits
# Then this handoff commit was authored on rec-c3-ci-hygiene, fast-forwarded
# back into main, and pushed. Subsequent corrections to this file
# (made across the same day) do not change the convergence invariant
# described in "Final remote state" above.
```

`gh pr edit 31` updated the PR description with real state before merge:
title "REC-C3.3.2 — Tren A (capability-bundle split + ratchet + checker
hygiene + infra fix)", body with base/head/46-commits/gate status.

## Decision rules carried forward (do not violate tomorrow)

1. **Tren B (REC-C3.3.3) MUST NOT start until `rec-c3-ci-hygiene` closes with all five gates GREEN.**
2. **No `dyn Any` / downcast / store helper / `chronos_store` DTO crossing the port** — B2 stop rule.
3. **No `NativeProbeServicePort`** — B1 reuses existing ports.
4. **`store→native` belongs to REC-C3.4, not Tren B.** Last waiver after Tren B is `store→native`; the final waiver closes only at REC-C3.4.
5. **Composition root stays in `chronos-mcp::composition`.** Bootstrap-scoped vs session-scoped factory distinction is mandatory.
6. **`SessionRepository` stays lifecycle/registry (no inflation).** B2 introduces `SessionArchive` (save/load/list/delete) + `CounterexampleRepository` (separate) + `SessionMetadata` moves to `chronos-domain`.
7. **No `#[ignore]` and no `--skip` ever.** Pre-existing failures get owner + cycle, not waivers.
8. **REC-C1.5.2 strict replay semantics stay untouched.** Slice A of hygiene reconciles the test + comment, not the production code.
9. **No squash, no rebase, no merge commit.** All cycles preserve their full SHA chain (operator invariant).
10. **Findings are `no_action_in_current_cycle`, not permanent waivers, when they describe scope-out work owned by a future cycle.** This is the new convention established today.

## Tomorrow's pre-flight

> The bash commands live in the **"Tomorrow's pre-flight (corrected)"**
> section near the top of this document. After running them, continue
> with the slice plan below.

Then:

1. Decide whether to drive the hygiene cycle inline or stop at a slice boundary.
2. Each slice (A, B, C) is A-min path (small, bounded, no architectural fork).
3. Per AGENTS.md §2: T0 + T1 minimum before each slice commit. Sandbox smoke (T4) is MANDATORY before merge per the matrix even on A-min for hygiene cycles — the whole point is to flip CI / Coverage / Vault Drift from RED to GREEN.
4. `CARGO_TARGET_DIR` and `CHRONOS_MCP_PATH` rules apply as usual.

## Artifact map (where to look tomorrow)

```
cycle-artifacts/p-3416cfb8288f8964/rec-c3-3-2-composition-integration-inversion/
├── apply-checkpoint.json      ← C3.3.2 final state, RELEASED
├── verify-findings.json       ← CC#18 evidence for Tren A
├── close-receipt.md           ← merge state + handoff to hygiene
└── (other artifacts)

cycle-artifacts/p-3416cfb8288f8964/rec-c3-ci-hygiene/
└── apply-checkpoint.json      ← OPEN, three slices defined

cycle-artifacts/p-3416cfb8288f8964/SESSION_HANDOFF_2026-09-18.md  ← morning handoff
cycle-artifacts/p-3416cfb8288f8964/session-handoff/               ← directory
```

## Key files for tomorrow's slices

### Slice A — m1_02 reconciliation

- Test (stale, to fix): `chronos-sandbox/tests/m1_acceptance.rs:197-235` (case 6)
- Production (REC-C1.5.2 strict, DO NOT TOUCH): `crates/chronos-log/src/segmented.rs:934-977` (`build_replay_plan` + `apply_plan`)
- Production (REC-C1.5.2 strict): `crates/chronos-log/src/replay.rs`
- Test docstring outdated: same file, comments at lines 197-198, 232

### Slice B — tripwire_depth harness

- Test: `chronos-sandbox/tests/tripwire_depth.rs` (lines 17, 36, 79, 115, 156, 196, 224, 257, 283, 319)
- Harness: `chronos-sandbox/src/client/tools.rs::McpTestClient::start` (path resolution)
- Coverage workflow: `.github/workflows/coverage.yml`
- Recipe per AGENTS.md: pre-build `cargo build --bin chronos-mcp`, set `CHRONOS_MCP_PATH`

### Slice C — Vault drift CC#18/19/39

- Sweep script: `scripts/check_vault_drift.sh`
- Check definitions: `scripts/vault_drift_checks/` (likely; locate exactly first)
- 9 cycles without `verify-findings.json` → identify them, add the file (template at `cycle-artifacts/.../rec-c3-3-2-.../verify-findings.json`)
- 6 free-text notes in `rec-c2.1` / `rec-c2.2` → migrate to structured fields
- Index count mismatch → inspect cycles-index, find drift, fix

## Quick links

- PR #31: https://github.com/Rubentxu/chronos/pull/31 (MERGED 2026-09-18T22:29:15Z)
- PR #31 last commit (Tren A code merge): `d30d0250`
- `origin/main` and `rec-c3-ci-hygiene`: run the verification command in
  the "Final remote state" section above to learn the current SHA. They
  must be equal.
- Session scratch handoff: `~/.jcode/scratch/handoff-2026-09-18-tren-a.md`
