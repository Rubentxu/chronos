# SANDBOX-S0.1a — Round 1 Results (host + bwrap)

Baseline: main `bcee2448`. Runner: `tools/s0run/s0run.py` (stdlib only).
Scenario: `scenarios/child-process-lifecycle.json` (parent → child → marker → exit).

## Command

```bash
python3 tools/s0run/s0run.py --scenario scenarios/child-process-lifecycle.json --env <host|bwrap> --rounds 2
```

## Results

| env | result | prepare_ms | execution_ms | cleanup_ms | leftover | pid_isolation |
|---|---|---|---|---|---|---|
| host | pass | 0.13 / 0.11 | 8.99 / 7.62 | 0.17 / 0.14 | [] | false |
| bwrap | pass | 0.12 / 0.12 | 30.98 / 25.42 | 0.16 / 0.14 | [] | true |

Reproducibility: stdout identical across rounds for both adapters.

## Findings (the point of the spike)

1. **Workspace must be an explicit writable bind.** With only `--ro-bind / /`,
   the fixture's marker write fails silently and the scenario reports
   `fail` with "stdout missing 'child-marker'". Fixed by
   `--bind <workdir> <workdir>`. Implication for the contract draft: the
   workspace is a first-class *input* (ExecutionSpec), not a backend detail.
2. **bwrap is ~3x slower than host for the same scenario** (26-31 ms vs 8-9 ms)
   but still sub-40 ms: the PROCESS class is cheap enough to be the default
   candidate for S0.4, consistent with the S0.1 characterization (bwrap 0.01 s
   process startup).
3. **`ebpf_available` is false on the host** (no `/sys/kernel/debug/tracing`
   here), so a KERNEL-class check cannot be validated until the QEMU path
   exists. Recorded as a capability unknown, not assumed.
4. **`remaining_processes` is a weak metric on host** (cannot attribute
   leftovers cheaply). The contract draft's `destroy()` verification needs a
   better story than "return 0": either cgroup/pidfd accounting (SYSTEM/KERNEL)
   or an explicit `unknown` verdict. Silent zero would be a Silent Lie.

## Not done (deliberately)

- podman/qemu adapters: only after host+bwrap agree on the scenario contract
  (S0.1b/c/d).
- No Chronos-on-Chronos: runner must be stable first (3 failure sources would
  be unattributable).
