# SANDBOX-S0.1 spike — first harness results (2026-09-16)

Branch: spike/sandbox-s0-characterization (rebased on post-REC-C0 main).
Harness: lab/run_scenario.sh + lab/scenarios/scenario.json. One logical
command (`true`), identical evidence shape per backend.

## Round 1: exit status + wall time

| backend | exit | duration_ms |
|---|---|---|
| host | 0 | 1 |
| bwrap (unshare-pid) | 0 | 12 |
| podman rootless | 0 | 4641 (includes runtime warm path) |
| qemu-kvm direct kernel boot | 0 | 1923 |

Consistent with S0.1 characterization on the release branch (bwrap ~10 ms,
podman ~300 ms warm after first pull, qemu ~2 s).

## Next probe rounds (planned, same harness shape)

R2. isolation facts: pid/mount/network namespace visibility per backend
    (readlink /proc/self/ns/* inside the environment).
R3. ptrace readiness: run a trivial PTRACE_TRACEME child per backend
    (expected: host+bwrap OK unprivileged; podman needs caps knob; qemu OK).
R4. eBPF/uprobe readiness: capability probe only (bpf() syscall gate).
R5. child-process + filesystem cleanliness: spawn/kill children, verify no
    leftovers and workspace removal post-run.
R6. evidence equivalence: run chronos-mcp probe_start/drain/stop inside
    each backend and diff evidence JSON shapes.

## Discard rule (per roadmap S0.8)

Any backend that cannot pass R2-R5 without privileged host mutation is
documented with its exact blocker and excluded from S0.4/S0.5 adapter work
until a named consumer UAT requires it.
