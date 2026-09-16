# SANDBOX-S0.1 Executable Spike — common scenario matrix

Status: SPIKE. Minimal harness, no framework. One scenario, four backends
(host, bwrap, podman, qemu-kvm). Discard before architecture if a backend
shows no measurable value.

## Scenario (single, identical everywhere)

`scenario.json`:

```json
{
  "scenario_id": "s0-child-echo",
  "steps": [
    {"cmd": ["sh", "-c", "echo hello-s0; sleep 0.1; echo done-s0"], "expect_stdout_contains": ["hello-s0", "done-s0"]},
    {"cmd": ["sh", "-c", "mkdir -p /tmp/s0-probe && touch /tmp/s0-probe/marker"], "expect": "success"},
    {"cmd": ["sh", "-c", "ls /tmp/s0-probe/marker"], "expect": "success"}
  ],
  "cleanup_required": ["/tmp/s0-probe"]
}
```

## Runner

`s0run` (single bash file, ~150 lines): reads scenario.json, dispatches to an
adapter by name, records JSON lines to results/<backend>.jsonl.

## Adapter measurements (matrix columns)

| Metric | How measured |
|---|---|
| prepare_time_ms | wall time before first step (bind/image/boot) |
| total_time_ms | first prepare step to last step |
| pid_isolation | `ps` inside sees only own tree? (host: no; bwrap: yes) |
| fs_isolation | writes outside workspace visible on host after run? |
| net_isolation | `wget -T1 127.0.0.1:1` behavior / netns absence (info only) |
| ptrace | `sh -c 'trap "" TRACE'` + attempt attach of `/bin/true` child (expect fail in bwrap default; record) |
| ebpf_uprobe_readiness | presence of /sys/kernel/debug/tracing (expect no inside bwrap/podman; yes in KVM guest) |
| child_processes | spawn 2 background children; count surviving after teardown |
| clean_fs | cleanup_required paths absent on host after destroy |
| teardown_time_ms | destroy/stop wall time |
| reproducibility | run twice; diff stdout captures |
| chronos_evidence_equiv | run chronos-mcp probe (if importable) OR record stdout+exit hash as evidence bundle hash |

## Backends

- `host`: direct exec (baseline).
- `bwrap`: `bwrap --ro-bind / / --dev /dev --proc /proc --tmpfs /tmp --unshare-pid --die-with-parent <steps>`.
- `podman`: `podman run --rm -v workspace:/w alpine sh -c '<steps>'` (rootless).
- `qemu`: prebuilt minimal kernel direct boot; steps injected via 9p/initrd
  script; one boot per scenario (S0.7 forbids optimizing boot).

## Exit criteria

- Matrix filled for all four backends on this host.
- Any backend with no column where it beats `host` AND no future requirement
  (KERNEL needs) gets a discard note, not code.
- Results land in `SANDBOX_S0_1_CHARACTERIZATION.md` appendix (separate commit
  on this spike branch only).
