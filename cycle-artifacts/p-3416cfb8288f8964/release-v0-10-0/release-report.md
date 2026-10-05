# release-report — v0.10.0

- cycle: `release-v0-10-0` (B-direct)
- main SHA: `57c36040f29ffea2df11335324037a35d393641c`
- tag: `v0.10.0` (annotated, object `b9fe0c4f`, peels to the same SHA)
- base: `v0.9.0` (`aac09659`) — 10 commits, 40 files
- runtime status at write time: RELEASE_PENDING, phase release

## What this release contains

The headline is a change of contract, and it exists because the previous
behaviour was a lie the API told on every successful drain.

`probe_drain` answered `"status": "running"` unconditionally. The empty state —
`from_seq 0`, `to_seq_exclusive 0`, `complete` — was byte-for-byte identical
whether the capture worker was healthy, dead, wedged in a blocking `waitpid`,
or had never started. A consumer had no way to tell, and a silent session looked
exactly like a quiet one.

It now reports one of seven states, and the answer rests on a fact the product
does not author: whether the tracee process still exists, asked of the kernel
with `kill(pid, 0)`. The `AtomicBool running` the backend already had is
deliberately **not** read. It is a control — "should I keep looping" — not an
observation, and reading it would reproduce the defect under a better name: the
product grading its own homework.

Precedence, documented on the port enum and restated in the code that honours it:

1. Tracee confirmed gone → `tracee_gone`, whatever the worker state says,
   including "the worker is inside its loop". That is the rule that stops the
   R6.5-class session reporting healthy with a frozen log.
2. Otherwise the worker's recorded state decides, and `capturing` is reachable
   only from a confirmed-alive tracee, never from "unknown".

`is_healthy()` is true only for `capturing`, the single value that means "this
capture is working".

## Two guards, because a fix with no guard is one edit from gone

**CC#57.** `CC#11`, `CC#22` and `CC#39` filter out cycle directories that
`git ls-files` reports as untracked. The filter is right — an orphaned working
tree directory is residue, not a cycle — but a cycle created and not yet
committed is *also* untracked, and while it was in that state the sweep still
printed PASS over a strictly smaller file set than CI would see. That is not
hypothetical: the local sweep was PASS during `release-v0-9-0`, and the first CI
run on the commit that added the directory came back red. CC#57 does not remove
the filter; it makes the skip visible.

**CC#58.** All three `panic!` paths in `rec_c2_2_uat_c2.rs` now tear the probe
sessions down before unwinding, because `Drop` cannot await `probe_stop`. CC#58
keeps it that way when someone adds a fourth path and forgets.

## Verification authority

| Level | Where | Result |
|---|---|---|
| Integration matrix (TIER 2) | remote CI Test job | 174 suites, 4125 tests, 0 failed, **0 deferred** |
| Workflows on this SHA | GitHub Actions | 6/6 success, 0 red |
| Targeted suites | local | probe_ports 13/13, native 120/120, services 609/609, mcp 115/115, probe_lifecycle 7/7, rec_c2_2_uat_c2 6/6 |
| Lint/format/manifests | local + pre-push | fmt clean, clippy `-D warnings` clean, CC#4 clean over 103 manifests |
| Vault gates | local + CI | sweep PASS (50 python CCs, 7 bash CCs), artifacts PASS over 33 cycles |

`uat_c2_01_probe_drain_is_not_an_authority` passed 17.66 s after its diagnostic
sibling; a skip would have burned the 300 s deadline, so it is a real pass. The
ancestor `666e2026` closed 7/7 the same way at 14.86 s.

## Non-vacuity, and a mutation that proved nothing

Every new guard was shown able to fail. For R6.6 the demonstration is that making
the worker's state outrank the tracee's existence turns three tests red, including
`liveness_does_not_report_capturing_when_the_tracee_has_exited` reporting
`left: Capturing / right: TraceeGone`. That guard uses a real process — `fork`,
`_exit`, `waitpid`, so the pid is genuinely reaped and `kill(pid, 0)` genuinely
answers `ESRCH` — with a healthy sibling case so it cannot be satisfied by always
answering `tracee_gone`.

The first mutation attempted was a **no-op**: reordering two disjoint `match`
arms changed nothing and the suite stayed green. It is recorded because that is
the most dangerous way to be wrong about a guard — declaring it non-vacuous
without checking that the mutation does anything at all.

## Honest limits

A worker wedged in `waitpid` behind a **live** tracee still reports `capturing`,
because the tracee genuinely exists. That is the honest answer to the question
asked, but it means liveness alone cannot detect that wedge. It is recorded as
`DEBT-WAIT-EVENT-UNBOUNDED-01` with the design and its CPU cost already
analysed: only the `follow_children` path of `wait_event` still blocks without
a bound, while the other path already polls with `WNOHANG` and a 10 ms sleep, so
the polling cost is a profile this repository already pays.

Also unverified and stated as such: the zombie window, where a killed but
unreaped tracee still answers `kill(pid, 0)`; the non-Linux arm of
`tracee_existence`, which compiles to `Unknown` by `cfg` but has never run; and
any real `wait_event` wedge, which would need ptrace privileges and a
deliberately stalled tracee.

## Release mechanics

`sddk release apply` converged with `applied: 1` and the capability receipt
`cap-git-tag-37884a637e7e` — the typed runner created and pushed the tag itself.
No manual Git step and no `--no-verify`. One working-tree change was held out on
purpose: the ledger note for `DEBT-WAIT-EVENT-UNBOUNDED-01`, stashed so the tag
would land on the SHA whose CI is green rather than on a commit that exists only
to carry documentation. It is committed immediately after the tag.
