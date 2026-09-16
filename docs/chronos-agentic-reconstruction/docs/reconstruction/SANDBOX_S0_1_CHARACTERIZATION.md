# SANDBOX-S0.1 — Environment Characterization (host: Fedora 44, local dev box)

Date: 2026-09-16. Scope: read-only smoke measurements. No abstraction implemented.
Status: preliminary evidence; NOT an acceptance gate for REC-C0.

## Versions

| Runtime | Version |
|---|---|
| Bubblewrap | 0.12.0 |
| Podman | 5.8.4 |
| QEMU | 10.2.2 (qemu-kvm same build) |
| libvirt/virsh | 12.0.0 |
| Kernel | 7.2.4-ogc3.1.fc44.x86_64 |

## Capability checks

| Check | Result |
|---|---|
| Unprivileged user namespaces (`unshare -U -r`) | OK |
| `/dev/kvm` accessible (crw-rw-rw-) | OK, 128 CPU vmx flags |
| bwrap minimal sandbox (`--ro-bind / / --dev /dev --proc /proc true`) | OK |
| Podman rootless container run (`quay.io/podman/hello`) | OK (first pull ~14 s warm-up) |

## Startup/teardown wall time (repeated runs)

| Runtime | Measured |
|---|---|
| Bubblewrap PROCESS | ~0.01 s per run |
| Podman SYSTEM | ~0.3 s warm (0.33, 0.26); 14.5 s first run including image pull |
| QEMU/KVM KERNEL | ~2.0 s to kernel panic exit (`-kernel` direct boot, 512 MB, no disk, `-enable-kvm`) |

Notes:
- QEMU first attempt failed on wrong kernel path; use `/usr/lib/modules/$(uname -r)/vmlinuz`. Recorded as a diagnostic finding (path resolution matters for S0.7).
- QEMU time measured to panic exit, not full userspace boot; treat as lower bound.

## Privilege requirements

| Class | Runtime | Privilege |
|---|---|---|
| PROCESS | Bubblewrap | unprivileged (userns) |
| SYSTEM | Podman | rootless |
| KERNEL | QEMU/KVM | unprivileged user; rw /dev/kvm (world-accessible here, but host-dependent) |

## Recommendation by scenario class (preliminary)

- PROCESS scenarios (isolate test subprocess, network off, fs subset): Bubblewrap, sub-10 ms overhead. Default candidate for S0.4 adapter.
- SYSTEM reproducibility (pin toolchain/OS deps): Podman rootless, ~0.3 s warm overhead. S0.5.
- KERNEL verification (ptrace/eBPF/uprobe in controlled kernel): QEMU/KVM direct kernel boot, ~2 s per ephemeral boot. Sufficient for S0.7 spike; no throughput work.

## Failure diagnostics observed

- Missing kernel image: QEMU fails fast (<0.1 s) with clear stderr. Good for CI error surfaces.
- First podman pull dominates latency; pre-pull or warm cache required for time-sensitive UAT.

## Follow-ups

1. Capture artifact collection story per backend (evidence bundle export paths).
2. Failure-mode tests: bwrap with missing bind target; podman with bad image; QEMU without /dev/kvm (TCG fallback measurement).
3. Equivalent logical scenario across the three backends (S0.2 contract input).
