# SANDBOX-S0.1a — Round 1 Results (host + bwrap), hardened to S0.1a+

Baseline: main `bcee2448`. Runner: `tools/s0run/s0run.py` (stdlib only).
Scenario: `scenarios/child-process-lifecycle.json` (parent → child → marker → exit).
Regression scenario: `scenarios/non-shell-command.json` (non-shell commands).

## Results

| env | result | prepare_ms | execution_ms | cleanup_ms | leftover |
|---|---|---|---|---|---|
| host | pass | ~0.1 | ~8 | ~0.15 | [] |
| bwrap | pass | ~0.1 | ~26-31 | ~0.15 | [] |

Reproducibility: stdout identical across rounds for both adapters.

## Findings from round 1 (before hardening)

1. **Workspace must be an explicit writable bind.** With only `--ro-bind / /`,
   the fixture's marker write fails silently and the scenario reports `fail`.
2. **bwrap ~3x host cost** (26-31 ms vs 8-9 ms), still sub-40 ms: PROCESS class
   stays the default candidate for S0.4.
3. **`ebpf_available` unproven** on this host (no `/sys/kernel/debug/tracing`).

## Hardening applied in S0.1a+ (three Silent Lies removed)

1. **`remaining_processes` no longer returns a fabricated `0`.** It is now an
   observation:
   ```json
   {"status":"unknown","value":null,"method":"unsupported-on-host",
    "reason":"no pid-namespace/cgroup ownership to attribute"}
   ```
2. **`ptrace` is no longer an assumed boolean.** A real probe
   (`probe_ptrace.py`, PTRACE_ATTACH on a live child then DETACH) provides
   provenance: `{"status":"supported","provenance":"probe:ptrace-attach/v1"}`.
   Yama/seccomp/permissions can deny attach even when the kernel supports it.
3. **No silent fallback to host in the bwrap adapter.** The old
   `if cmd[0] == "sh"` guard meant any other command (`python`, `cargo`,
   `chronos-mcp`, `./fixture`) ran on the HOST while the result claimed bwrap
   isolation. `wrap()` now wraps every command and returns `None` (error step)
   rather than falling back.

   Differential proof (`non-shell-command.json`, `/bin/echo` + pid count):
   ```text
   host  | ['direct-hello', '1151']   # sees the whole host process table
   bwrap | ['direct-hello', '4']      # sees 4 pids inside its own namespace
   ```

## Capability vocabulary (tri-state)

```text
supported | unsupported | unknown
```

Every entry carries provenance (a probe id) or a reason. Nothing is `true` by
default. Chronos evidence classes are deliberately NOT imported here.

## Workspace model

```text
source_ro       read-only source tree        (never writable for a fixture's convenience)
work_rw         writable scratch for the scenario
artifacts_rw    writable outputs to be collected
tmp_ephemeral   tmpfs, discarded with the environment
```

bwrap binds: `--ro-bind / /`, `--bind work_rw`, `--bind artifacts_rw`,
`--ro-bind source_ro`, `--tmpfs tmp_ephemeral`, `--unshare-pid`, `--die-with-parent`.

## Not done (deliberately)

- podman/qemu adapters: S0.1b/c/d, only after host+bwrap agree (they do).
- `collect()` stays out of the environment adapter: the environment produces
  artifacts; interpreting them as evidence is a separate responsibility (to be
  confirmed by Podman/QEMU, not decided by fiat).
