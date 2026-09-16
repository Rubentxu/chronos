# SANDBOX-S0.1b — Podman (rootless) Results

Same scenario (`child-process-lifecycle.json`) and same result schema as S0.1a.
Baseline: main `bcee2448`. Podman 5.8.4, rootless, image `alpine:3.20`.

## Placement comparison (whole scenario, steady state)

| env | result | execution_ms (3 runs) | notes |
|---|---|---|---|
| host | pass | 9.8 / 9.3 / 9.3 | one subprocess per step |
| bwrap | pass | 39.2 / 32.2 / 30.7 | one bwrap per step |
| podman | pass | 23526 / 735 / 623 | one container for the whole scenario |

Observable semantics are identical: same steps, same exits, same stdout
payloads. Placement changed the cost, not the meaning.

## Findings

1. **One container per step is unusable.** The first Podman implementation ran
   `podman run` per step: ~11 s per step, ~23 s for the scenario. That is three
   orders of magnitude above the host and would make any comparison noise.
   The adapter now executes the whole scenario in a single container session
   with step delimiters. Same assertions, same schema — a placement decision,
   not a semantic one.
2. **SELinux Enforcing breaks bind mounts without a relabel.** On this Fedora
   host, `-v ...:rw` was denied (`Permission denied` writing the marker) until
   `:z` was added. Two consequences: (a) a Podman adapter must know about host
   LSM policy, (b) `:z` **relabels the host directory**, so the SYSTEM class
   mutates host state as a side effect of mounting. Recorded, not hidden.
3. **Podman cost is bimodal.** Warm runs are 620-740 ms; occasionally a run
   takes ~23 s (registry/image resolution). Any time budget built on Podman
   must treat the slow mode as normal, not as a failure.
4. **`remaining_processes` is now honestly `supported` for Podman** (value 0,
   method "--rm + clean exit") because that is an actual contract, unlike the
   host where attribution is impossible and the value stays `unknown`.

## Negative tests (`tests/negative.sh`, all PASS)

| # | Case | Required behaviour | Result |
|---|---|---|---|
| N1 | podman unavailable (`S0_FORCE_UNAVAILABLE=podman`) | explicit `skip` + reason, no metrics | PASS |
| N2 | broken image/mount (`S0_PODMAN_IMAGE=...nonexistent`) | hard `fail` with container error; host output must NOT appear | PASS |
| N3 | host and bwrap results | labelled with their actual environment | PASS |

N2 is the important one: it proves there is no silent fallback to the host when
the container refuses to start. At no point does a failed Podman run produce
host-executed output.

## Isolation configuration used (and deliberately not used)

```text
--network=none        network disabled
--read-only           read-only rootfs
--rm                  no leftover container
--tmpfs /run          writable runtime dir
source_ro :ro / work_rw :rw,z / artifacts_rw :rw,z / tmp_ephemeral :rw,z
```

Not used on purpose: `--privileged`, `SYS_PTRACE`, eBPF/uprobe capabilities.
S0.1b compares placement; privileged capabilities would add a second variable.
Note that `ptrace` therefore reports `unsupported`/unproven inside Podman, which
is exactly the capability asymmetry S0.7 (KERNEL class) exists to serve.

## Decision input for S0.8

- Podman earns its place for SYSTEM-class reproducibility, not for speed.
- Its cost is acceptable for scenario-level suites, not for per-test isolation.
- The `:z` host relabel side effect must be part of any capability disclosure.
