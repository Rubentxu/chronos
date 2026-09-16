# SANDBOX-S0.1c — QEMU/KVM Results (KERNEL class)

Same scenario (`child-process-lifecycle.json`), same assertions, same result
schema as host / bwrap / podman. Baseline: main `bcee2448`.

## The gate this closes

```text
host ─────┐
bwrap ────┤
podman ───┼─ same Scenario Contract
qemu/kvm ─┘
```

Four placements, one scenario, one semantics. Privileged probes (ptrace/eBPF in
the guest) are deliberately **not** attempted here: that is S0.7.

## Comparison (steady state)

| env | result | prepare_ms | execute_ms | cleanup_ms | leftovers |
|---|---|---|---|---|---|
| host | pass | 0.4 | 8.4 | 0.6 | [] |
| bwrap | pass | 0.4 | 33.9 | 0.4 | [] |
| podman | pass | 22490 (cold) / ~56 (warm) | 268.7 | 57.3 | [] |
| qemu | pass | 1494.7 (cold assets) / cached after | 2178.5 | 0.5 | [] |

Observable semantics are identical across all four: the same steps, the same
exit codes, the same stdout payloads, the same artifacts.

## How the KERNEL environment is built

No VM framework, no cloud image, no disk:

```text
local container image (already present, no network)
        │  podman export
        ▼
read-only base initramfs  (hashed, cached, never written during a run)
        │  + scenario.sh injected into a per-run COPY
        ▼
qemu-kvm -enable-kvm -kernel <host vmlinuz> -initrd <per-run initramfs>
         -append 'console=ttyS0 rdinit=/init panic=-1' -nographic -no-reboot
        │
        ├── guest ramfs = the ephemeral overlay (discarded at poweroff)
        ├── readiness  = guest prints __S0_READY__ (never a sleep)
        └── artifacts  = tar|base64 over the serial console (copy-out)
```

Properties that satisfy the S0.1c requirements:

- **Base read-only**: the cached initramfs is never mutated; each run gets a
  fresh copy with the scenario injected. The guest's writable layer is its own
  ramfs.
- **No host shares**: there is no virtio-9p / no disk image / no bind mount. The
  scenario is copied in at prepare time; artifacts come back over serial.
- **Boot readiness is real**: the host waits for `__S0_READY__` from the guest
  init and fails the run if it never arrives ("guest never reported readiness;
  no results accepted") instead of sleeping a guessed duration.
- **Cleanup verified**: no QEMU process survives, and the per-run initramfs and
  its staging directory are removed (`leftover_paths: []`).
- **Provenance**: QEMU version, host kernel path + SHA-256, per-run initramfs
  SHA-256, accelerator.

## No TCG fallback

If `/dev/kvm` is not usable, the adapter reports `unsupported`. There is no
silent fallback to TCG: TCG is a *different placement* with different timing and
different kernel semantics, and it would need its own characterization rather
than being smuggled in as "the same environment".

## Negative tests (added N7-N9, all PASS)

| # | Case | Result |
|---|---|---|
| N7 | KVM forced unavailable | PASS (`kvm_available()` false; unsupported path reachable) |
| N8 | QEMU provenance + cleanup | PASS (accelerator=kvm, kernel and initramfs hashed, no leftovers) |
| N9 | Podman executes by immutable image id | PASS (never by mutable tag) |

## Findings

1. **A host kernel boots fine as a guest kernel** with an initramfs-only root,
   which avoids downloading any cloud image and keeps the whole environment
   offline and reproducible from locally present assets.
2. **The readiness marker removed a whole class of flakiness.** The first
   implementation read the serial stream to completion; keying on the guest's
   own `__S0_READY__` turned boot timing into a protocol instead of a guess.
3. **SELinux also applies to the exported rootfs**: `podman export` output
   contains absolute-path symlinks that Python's default safe extractor refuses;
   extraction here is explicitly `fully_trusted` on assets we produced locally.
4. **Cost profile is now clean for all four classes**: PROCESS ~34 ms, SYSTEM
   ~0.3 s warm, KERNEL ~2.2 s per ephemeral boot (S0.7 explicitly forbids
   optimizing boot throughput in S0).

## Consequence for the contract draft

The four placements differ in cost and isolation, but not in the shape they
consume: an input (source, staged), an ephemeral work area, an output
(artifacts, copied out), and a verified teardown. That reinforces the S0.1b+
conclusion: `ExecutionEnvironment` should expose declarative inputs/outputs,
not mounts.
