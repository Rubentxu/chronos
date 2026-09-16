#!/usr/bin/env python3
"""SANDBOX-S0.1c — QEMU/KVM assets for the S0 execution model.

Builds a minimal initramfs from a LOCALLY PRESENT container image (no network),
so the KERNEL-class environment can run the same scenario as host/bwrap/podman.

Design constraints (from the S0.1c gate):
  * base is read-only: the initramfs file is never written to during a run;
  * the ephemeral overlay is the guest's own ramfs, discarded at poweroff;
  * no host shares: the scenario script is copied IN at prepare time and the
    artifacts come back over the serial console (copy-in / copy-out only);
  * boot readiness is a marker printed by the guest, never a sleep;
  * the accelerator must be KVM; without it the environment is `unsupported`,
    there is no silent TCG fallback.
"""
from __future__ import annotations

import hashlib
import io
import os
import shutil
import subprocess
import tarfile
from pathlib import Path

READY_MARK = "__S0_READY__"
BEGIN_MARK = "__S0_BEGIN__"
END_MARK = "__S0_END__"
ART_MARK = "__S0_ARTIFACTS_B64__"

INIT_SCRIPT = f"""#!/bin/sh
# S0.1c guest init: bring up the minimum, announce readiness, run the injected
# scenario, stream artifacts out, then power off. No host filesystem is shared.
mount -t proc none /proc
mount -t sysfs none /sys 2>/dev/null
mount -t tmpfs none /tmp
mkdir -p /s0/work /s0/artifacts /s0/tmp
cd /s0/work
echo "{READY_MARK}"
if [ -x /s0/scenario.sh ]; then
    . /s0/scenario.sh
fi
tar cf - -C /s0/artifacts . 2>/dev/null | base64 -w0
echo ""
echo "{ART_MARK}"
poweroff -f
"""


def _sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def qemu_binary() -> str | None:
    for name in ("qemu-kvm", "qemu-system-x86_64"):
        found = shutil.which(name)
        if found:
            return found
    return None


def qemu_version(binary: str) -> str:
    proc = subprocess.run([binary, "--version"], capture_output=True, text=True)
    return (proc.stdout or proc.stderr).strip().splitlines()[0] if proc.returncode == 0 else "unknown"


def kvm_available() -> bool:
    """KVM must be present AND usable. No accelerator => unsupported."""
    if os.environ.get("S0_FORCE_KVM_UNAVAILABLE"):
        # Negative-test hook: proves the unsupported path is reachable and that
        # nothing silently degrades to TCG.
        return False
    dev = Path("/dev/kvm")
    return dev.exists() and os.access(dev, os.R_OK | os.W_OK)


def host_kernel() -> Path | None:
    mods = Path("/usr/lib/modules")
    if not mods.exists():
        return None
    for entry in sorted(mods.iterdir(), reverse=True):
        candidate = entry / "vmlinuz"
        if candidate.exists():
            return candidate
    return None


def build_base_initramfs(
    image: str, cache_dir: Path, scenario_script: str | None = None
) -> tuple[Path | None, str, str]:
    """Materialise the initramfs. Returns (path, sha256, note).

    The base (image rootfs + init) is cached and reused unless a scenario must
    be injected, in which case a per-run copy carries `/s0/scenario.sh`. The
    base is never mutated by a run.
    """
    cache_dir.mkdir(parents=True, exist_ok=True)
    base = cache_dir / "s0-base.cpio.gz"
    rootfs = cache_dir / "rootfs"
    note = ""

    if not rootfs.exists() or not base.exists():
        container = f"s0-initramfs-{os.getpid()}"
        subprocess.run(["podman", "rm", "-f", container], capture_output=True)
        create = subprocess.run(
            ["podman", "create", "--name", container, image, "true"],
            capture_output=True, text=True,
        )
        if create.returncode != 0:
            return None, "", f"image {image} not usable locally: {create.stderr.strip()[:200]}"
        try:
            export = subprocess.run(
                ["podman", "export", container], capture_output=True, check=False
            )
            if export.returncode != 0:
                return None, "", f"podman export failed: {export.stderr.decode()[:200]}"
            if rootfs.exists():
                shutil.rmtree(rootfs)
            rootfs.mkdir(parents=True)
            with tarfile.open(fileobj=io.BytesIO(export.stdout), mode="r") as tf:
                tf.extractall(rootfs, filter="fully_trusted")
            init = rootfs / "init"
            init.write_text(INIT_SCRIPT)
            init.chmod(0o755)
            (rootfs / "s0").mkdir(exist_ok=True)
            _pack(rootfs, base)
        finally:
            subprocess.run(["podman", "rm", "-f", container], capture_output=True)

    if scenario_script is None:
        return base, _sha256_file(base), note

    # Per-run initramfs: copy the cached rootfs, inject the scenario, repack.
    # The base (cached rootfs + s0-base.cpio.gz) is never mutated by a run.
    run_dir = cache_dir / f"run-{os.getpid()}-{abs(hash(scenario_script)) % 10**6}"
    if run_dir.exists():
        shutil.rmtree(run_dir)
    shutil.copytree(rootfs, run_dir, symlinks=True)
    script = run_dir / "s0" / "scenario.sh"
    script.write_text(scenario_script)
    script.chmod(0o755)
    out = run_dir.with_suffix(".cpio.gz")
    _pack(run_dir, out)
    return out, _sha256_file(out), note


def _pack(rootfs: Path, out: Path) -> None:
    filelist = "\n".join(
        str(p.relative_to(rootfs)) for p in sorted(rootfs.rglob("*"))
    )
    cpio = subprocess.run(
        ["cpio", "-o", "-H", "newc", "--quiet"],
        cwd=rootfs, input=filelist.encode(), capture_output=True, check=True,
    )
    gz = subprocess.run(["gzip", "-9"], input=cpio.stdout, capture_output=True, check=True)
    out.write_bytes(gz.stdout)
