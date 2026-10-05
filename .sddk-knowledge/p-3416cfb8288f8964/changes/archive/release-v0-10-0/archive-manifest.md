# Archive Manifest — release-v0-10-0

## Summary

Release of `v0.10.0`. The headline is a change of contract that exists because
the previous behaviour was a lie the API told on every successful drain.

`probe_drain` answered `"status": "running"` unconditionally. The empty state —
`from_seq 0`, `to_seq_exclusive 0`, `complete` — was byte-for-byte identical
whether the capture worker was healthy, dead, wedged in a blocking `waitpid`, or
had never started. It now reports one of seven states, and the answer rests on a
fact the product does not author: whether the tracee process still exists, asked
of the kernel with `kill(pid, 0)`.

The release also adds two vault checks, both born of gates that had been green
for the wrong reason: `CC#57`, because a cycle that is not yet committed fell
outside the scope of the CCs that filter on `git ls-files`, and `CC#58`, because
a fix that nothing checks is one edit from gone.

## Identification

| Field | Value |
|---|---|
| Cycle | `release-v0-10-0` |
| Date | 2026-10-05 |
| Path | B-direct |
| Base tag | `v0.9.0` (`aac09659`) |
| Head SHA | `57c36040f29ffea2df11335324037a35d393641c` |
| Tag | `v0.10.0` |
| Tag object | `b9fe0c4fa116504fb89e448e183f289b65793fa7` |
| Tag peel | `57c36040f29ffea2df11335324037a35d393641c` |
| Range | `v0.9.0..57c36040`, 10 commits, 40 files |
| Verdict | `PARTIAL` (native `PASS_WITH_WARNINGS`) |

## Chain of custody

`release-receipt` -> `archive-manifest`, both bound to `57c36040`.

## The change

| Aspect | Value |
|---|---|
| Port | `NativeProbeController::liveness()` in `chronos-domain/src/ports/probe.rs` |
| Type | `ProbeLiveness`: `Capturing`, `Starting`, `NotStarted`, `TraceeGone`, `WorkerFinished`, `WorkerStopped`, `WorkerFailed` |
| Independent signal | `kill(pid, 0)`: `ESRCH` → gone, `EPERM`/`Ok` → alive, anything else → unknown, never guessed into alive |
| Deliberately not read | `AtomicBool running` — a control, not an observation |
| Precedence | tracee gone outranks worker state, including "worker is inside its loop" |
| Healthy value | `capturing`, reachable only from a confirmed-alive tracee |

Reading the control flag would have reproduced the defect under a better name:
the product grading its own homework. That is the whole reason the enum names
what a caller can *observe* rather than what the product intended.

## Two guards, and how they were shown able to fail

`CC#57` — a cycle directory with `apply-checkpoint.json` and nothing tracked by
git is now DRIFT. With everything tracked the sweep is PASS (50 python CCs, 7
bash CCs); creating a work-in-progress cycle turns it red. In that same run
`CC#11`, `CC#22` and `CC#39` stayed green, which is exactly the gap: they are
the three that filter on `git ls-files`.

`CC#58` — every `panic!` path in `rec_c2_2_uat_c2.rs` must have a teardown
within the 12 lines above it. Removing one teardown turns the sweep red. The
window is deliberately narrow: widening it until the check cannot fail is the
same mistake as deleting it.

For the liveness change itself, making the worker's state outrank the tracee's
existence turns three tests red, including
`liveness_does_not_report_capturing_when_the_tracee_has_exited` reporting
`left: Capturing / right: TraceeGone`. That guard uses a real process — `fork`,
`_exit`, `waitpid` — so the pid is genuinely reaped and `kill(pid, 0)` genuinely
answers `ESRCH`, with a healthy sibling case so it cannot be satisfied by always
answering `tracee_gone`.

**A mutation that proved nothing, recorded on purpose.** The first attempt
reordered two disjoint `match` arms, which is a semantic no-op: the suite stayed
green. It is written down because declaring a guard non-vacuous without checking
that the mutation does anything is the most dangerous way to be wrong about it.

## Verification authority, stated precisely

| Level | Where | Result |
|---|---|---|
| Integration matrix (TIER 2) | remote CI Test job | 174 suites, 4125 tests, 0 failed, 0 deferred |
| Workflows on this SHA | GitHub Actions | 6/6 success, 0 red |
| Ancestor `666e2026` | GitHub Actions | 7/7 success, 0 deferred |
| Targeted suites | local | 13 + 120 + 609 + 115 + 7 + 6 tests green |
| Vault gates | local + CI | sweep PASS, artifacts PASS over 34 cycles, CC#4 clean over 103 manifests |

`uat_c2_01_probe_drain_is_not_an_authority` passed 17.66 s after its diagnostic
sibling; a skip would have burned the 300 s deadline, so it is a real pass. The
change of contract did not weaken the test that had been built to stop trusting
the product's own claim.

## Cross-checks

- `python3 scripts/regen_manifest_index_shas.py --check`: clean.
- `bash scripts/check_vault_drift.sh`: PASS (50 python CCs, 7 bash CCs).
- `python3 scripts/validate_cycle_artifacts.py`: PASSED on 34 cycles.
- `sddk ledger verify`: valid.
- `sddk release plan --tag v0.10.0`: `version_authority: cross_checked`.

## A mistake caught while bumping the version

The bump regex changed 21 lockfile lines to `0.10.0`, two of which were
third-party crates that happened to sit at `0.9.0`: `rand_chacha` and
`untrusted`. A lockfile pointing at versions the registry does not have
corrupts the dependency graph quietly, and `cargo check` accepts it either way,
so the only signal is noticing *which* crates moved. They were restored, and
`cargo check` confirmed cargo did not rewrite the lock — leaving exactly the 19
workspace lines changed.

## Open at archive

`DEBT-WAIT-EVENT-UNBOUNDED-01`. Only the `follow_children` path of `wait_event`
still blocks without a bound; the other path already polls with `WNOHANG` and a
10 ms sleep, so the polling cost is a profile this repository already pays. The
design and its CPU trade-off are analysed in the debt ledger. A worker wedged
behind a **live** tracee therefore still reports `capturing`, which is the honest
answer to the question asked and the reason the verdict is `PARTIAL` rather than
a clean pass.

Also stated as unverified in the verification report: the zombie window where a
killed but unreaped tracee still answers `kill(pid, 0)`; the non-Linux arm of
`tracee_existence`, compiled by `cfg` but never run; and any real `wait_event`
wedge, which would need ptrace privileges and a deliberately stalled tracee.

## Index SHAs

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/release-v0-10-0/archive-manifest.md` | `0000000000000000000000000000000000000000000000000000000000000000` |
| implementation-receipt | `cycle-artifacts/p-3416cfb8288f8964/release-v0-10-0/implementation-receipt.json` | `1aec2efa0e1252b97224afbbd395edb12ac3c2e5393fdfbeef252c28a6582643` |
| verification-report | `cycle-artifacts/p-3416cfb8288f8964/release-v0-10-0/verification-report.json` | `e8020fe610db4c520dbaf62f3de57523a61b46087b43adbc281145a02c08d7e2` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/release-v0-10-0/verify-findings.json` | `fe72d7b855d1cc65a4121f0c35b6ef911849a855e6410cf7b06742555374fc8f` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/release-v0-10-0/release-receipt.md` | `72ca31f0886c0972466355013e1fe0b3a71bf0a2ac5ff333003452b981648a2d` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/release-v0-10-0/merge-receipt.md` | `58b4affcb71c108510cc02fa864c4f4a70e8dcd58b2cf3193d5cba6c47a3972f` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/release-v0-10-0/release-report.md` | `e5c7e33ddd9587ab721a8d3d49e317c33ad45f871a9b5ede7a07123d69b9f730` |
