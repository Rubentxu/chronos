#!/usr/bin/env python3
"""s0run — minimal SANDBOX-S0.1a+ scenario runner.

One scenario, one adapter at a time. Emits a machine-readable JSON result.
No framework: this exists to produce the measurement matrix that will decide
whether the ExecutionEnvironment contract draft survives contact with reality.

Design rules learned in S0.1a (see RESULTS-S0.1a.md):

* Capabilities are TRI-STATE. `supported | unsupported | unknown` with
  provenance. A capability that was not measured is NEVER reported as true.
  "capability not measured != capability supported".
* `remaining_processes` is an Observation, not an integer. Fabricating a zero
  when the adapter cannot attribute leftovers is a Silent Lie.
* A bwrap command is either run INSIDE bwrap or reported as an error. There is
  no silent fallback to the host.
* The workspace is modelled explicitly:
      source_ro  | work_rw | artifacts_rw | tmp_ephemeral
  The source tree is never writable just because a fixture wants a marker.
"""
from __future__ import annotations

import argparse
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import qemu_assets as qa

# S0.2: the contract belongs to Chronos, not to the SDDK workflow that
# governs development. Renamed BEFORE it has consumers, so no external API
# migration is ever needed. `chronos.execution.*` also fits the future
# Portable Execution Runtime, which is not a sandbox.
SCENARIO_SCHEMA = "chronos.execution.scenario/v1"
RESULT_SCHEMA = "chronos.execution.result/v1"

# Capability status vocabulary. Deliberately tiny; Chronos evidence classes are
# NOT imported here — the spike stays light.
SUPPORTED = "supported"
UNSUPPORTED = "unsupported"
UNKNOWN = "unknown"


def cap(status: str, **extra) -> dict:
    d = {"status": status}
    d.update(extra)
    return d


def observation(status: str, value=None, method: str | None = None, reason: str | None = None) -> dict:
    """Tri-state observation: supported(value+method) | unsupported(reason) | unknown(reason)."""
    d = {"status": status, "value": value, "method": method}
    if reason is not None:
        d["reason"] = reason
    return d


def which(name: str) -> str | None:
    return shutil.which(name)


def _available(name: str, probe) -> bool:
    """Availability probe. `S0_FORCE_UNAVAILABLE` exists so the negative tests
    can prove that an unavailable environment yields an explicit
    skip/unsupported result instead of silently degrading to the host."""
    forced = os.environ.get("S0_FORCE_UNAVAILABLE", "")
    if name in [x.strip() for x in forced.split(",") if x.strip()]:
        return False
    return probe()


ADAPTERS = {
    "host": lambda: _available("host", lambda: True),
    "bwrap": lambda: _available("bwrap", lambda: which("bwrap") is not None),
    "podman": lambda: _available("podman", lambda: which("podman") is not None),
    "qemu": lambda: _available(
        "qemu", lambda: which("qemu-kvm") is not None or which("qemu-system-x86_64") is not None
    ),
}


def load_scenario(path: Path) -> dict:
    with path.open() as fh:
        doc = json.load(fh)
    if doc.get("schema") != SCENARIO_SCHEMA:
        raise SystemExit(f"unsupported scenario schema: {doc.get('schema')!r}")
    return doc


def expand(s: str, env: dict[str, str]) -> str:
    out = s
    for k, v in env.items():
        out = out.replace(f"${k}", v)
    return out


class Workspace:
    """Explicit workspace model. Each region has a distinct contract."""

    def __init__(self, root: Path):
        self.root = root
        self.source_ro = root / "source_ro"
        self.work_rw = root / "work_rw"
        self.artifacts_rw = root / "artifacts_rw"
        self.tmp_ephemeral = root / "tmp_ephemeral"

    def materialize(self) -> None:
        for d in (self.source_ro, self.work_rw, self.artifacts_rw, self.tmp_ephemeral):
            d.mkdir(parents=True, exist_ok=True)

    def marker_path(self) -> Path:
        return self.work_rw / "child-marker.txt"

    def env(self) -> dict[str, str]:
        return {
            "S0_SOURCE": str(self.source_ro),
            "S0_WORK": str(self.work_rw),
            "S0_ARTIFACTS": str(self.artifacts_rw),
            "S0_TMP": str(self.tmp_ephemeral),
            "S0_MARKER": str(self.marker_path()),
        }


class HostAdapter:
    """Reference implementation. Everything else must match this shape."""

    name = "host"

    def __init__(self, ws: Workspace):
        self.ws = ws
        self._scenario_steps: list = []

    def configure(self, scenario: dict) -> None:
        """Hook for adapters that must know the scenario during prepare()."""
        self._scenario_steps = scenario["steps"]
        self._marker_probe_ok: bool | None = None

    def capabilities(self) -> dict:
        # Nothing here is assumed. ptrace/ebpf are probed, not declared.
        return {
            "pid_isolation": cap(UNSUPPORTED, reason="host runs in the caller pid namespace"),
            "fs_isolation": cap(UNSUPPORTED, reason="host sees the real filesystem"),
            "net_isolation": cap(UNSUPPORTED, reason="host shares the caller network namespace"),
            "ptrace": self._probe_ptrace(),
            "ebpf": cap(UNKNOWN, reason="not-probed"),
            "root_required": cap(SUPPORTED, evidence="runs as uid %d" % os.getuid()),
            "kernel_class": cap(UNSUPPORTED, reason="host kernel, not an ephemeral VM"),
        }

    def _probe_ptrace(self) -> dict:
        # Real probe: spawn a child and try to attach. Yama/seccomp/permissions
        # can all block this even when the kernel supports ptrace.
        child = subprocess.Popen(["sleep", "1"])
        try:
            probe = subprocess.run(
                [sys.executable, str(Path(__file__).parent / "probe_ptrace.py"), str(child.pid)],
                capture_output=True, text=True, timeout=10,
            )
            if probe.returncode == 0:
                return cap(SUPPORTED, provenance="probe:ptrace-attach/v1")
            return cap(
                UNSUPPORTED,
                provenance="probe:ptrace-attach/v1",
                reason=(probe.stdout.strip() or probe.stderr.strip() or "attach denied"),
            )
        except Exception as exc:  # noqa: BLE001
            return cap(UNKNOWN, reason=f"probe-error: {exc}")
        finally:
            child.kill()
            child.wait()

    def prepare(self) -> tuple[float, dict]:
        t0 = time.monotonic()
        self.ws.materialize()
        dt = (time.monotonic() - t0) * 1000.0
        return dt, {
            "source_ro": str(self.ws.source_ro),
            "work_rw": str(self.ws.work_rw),
            "artifacts_rw": str(self.ws.artifacts_rw),
            "tmp_ephemeral": str(self.ws.tmp_ephemeral),
        }

    def wrap(self, cmd: list[str]) -> list[str]:
        """Run a command in this environment. Must never silently fall back."""
        return cmd

    def execute(self, scenario: dict, env: dict[str, str]) -> tuple[float, list[dict]]:
        results = []
        t0 = time.monotonic()
        for step in scenario["steps"]:
            raw = [expand(a, env) for a in step["run"]]
            cmd = self.wrap(raw)
            if cmd is None:
                results.append({
                    "step": step["id"], "exit": None, "elapsed_ms": 0.0,
                    "stdout": "", "stderr": "",
                    "errors": [f"adapter {self.name} cannot execute {raw[0]!r}: unsupported"],
                })
                continue
            started = time.monotonic()
            proc = subprocess.run(
                cmd, cwd=self.ws.work_rw, env={**os.environ, **env},
                capture_output=True, text=True, timeout=step.get("timeout_s", 60),
            )
            elapsed = (time.monotonic() - started) * 1000.0
            errs = []
            for needle in step.get("expect_stdout_contains", []):
                if needle not in proc.stdout:
                    errs.append(f"stdout missing {needle!r}")
            expected_exit = step.get("expect_exit", 0)
            if proc.returncode != expected_exit:
                errs.append(f"exit {proc.returncode} != {expected_exit}")
            results.append({
                "step": step["id"], "exit": proc.returncode,
                "elapsed_ms": round(elapsed, 2),
                "stdout": proc.stdout, "stderr": proc.stderr, "errors": errs,
                "executed_as": cmd[:1],
            })
        dt = (time.monotonic() - t0) * 1000.0
        return dt, results

    def remaining_processes(self) -> dict:
        # Cannot attribute leftovers cheaply on the host. Report that honestly.
        return observation(UNKNOWN, value=None, method="unsupported-on-host",
                           reason="no pid-namespace/cgroup ownership to attribute")

    def destroy(self, cleanup_paths: list[str]) -> tuple[float, list[str]]:
        t0 = time.monotonic()
        leftover = []
        for p in cleanup_paths:
            path = Path(expand(p, {**os.environ, **self.ws.env()}))
            try:
                if path.is_file():
                    path.unlink()
                elif path.is_dir():
                    shutil.rmtree(path)
            except FileNotFoundError:
                pass
            if path.exists():
                leftover.append(str(path))
        dt = (time.monotonic() - t0) * 1000.0
        return dt, leftover


class BwrapAdapter(HostAdapter):
    name = "bwrap"

    def capabilities(self) -> dict:
        caps = super().capabilities()
        caps.update({
            "pid_isolation": cap(SUPPORTED, provenance="--unshare-pid"),
            "fs_isolation": cap(SUPPORTED, provenance="--ro-bind / / + explicit rw binds"),
            "net_isolation": cap(UNSUPPORTED, reason="no --unshare-net in this adapter yet"),
            "kernel_class": cap(UNSUPPORTED, reason="shares the host kernel"),
        })
        return caps

    def wrap(self, cmd: list[str]) -> list[str] | None:
        """Wrap ANY command. No `cmd[0] == "sh"` special case: that path used to
        run non-shell commands on the host while claiming bwrap isolation."""
        w = self.ws
        return [
            "bwrap",
            "--ro-bind", "/", "/",
            "--bind", str(w.work_rw), str(w.work_rw),
            "--bind", str(w.artifacts_rw), str(w.artifacts_rw),
            "--ro-bind", str(w.source_ro), str(w.source_ro),
            "--tmpfs", str(w.tmp_ephemeral),
            "--dev", "/dev",
            "--proc", "/proc",
            "--unshare-pid",
            "--die-with-parent",
            *cmd,
        ]



class PodmanAdapter(HostAdapter):
    """Rootless Podman: SYSTEM class. Same scenario, same assertions, same
    result schema. No --privileged, no SYS_PTRACE, no eBPF capabilities.

    S0.1b+ design rules (from the S0.1b review):

    * `prepare()` resolves the image digest LOCALLY and refuses if absent.
      No registry contact. `execute()` runs with `--pull=never`, so a test can
      never stall ~23 s on registry resolution.
    * NO bind mounts. The S0.1b SELinux finding showed that `:z` relabels the
      HOST directory, which is an unacceptable property for an environment we
      advertise as reproducible and clean. Instead the workspace is STAGED:
          source_ro -> tar stream in
          work/artifacts/tmp -> container tmpfs
          artifacts -> tar stream out
      This also matches how a remote worker (no local filesystem) must work.
    * Inputs/outputs are declarative; the backend decides materialization.
    * Cleanup is verified, not assumed: the container name must be gone.
    """

    name = "podman"

    @property
    def image(self) -> str:
        return os.environ.get("S0_PODMAN_IMAGE", "docker.io/library/alpine:3.20")

    def capabilities(self) -> dict:
        caps = super().capabilities()
        caps.update({
            "pid_isolation": cap(SUPPORTED, provenance="container pid namespace"),
            "fs_isolation": cap(SUPPORTED, provenance="read-only rootfs + staged inputs/outputs"),
            "net_isolation": cap(SUPPORTED, provenance="--network=none"),
            "kernel_class": cap(UNSUPPORTED, reason="shares the host kernel"),
        })
        return caps

    def resolve_image(self) -> tuple[str | None, dict]:
        """Local-only resolution to an IMMUTABLE execution identity.

        S0.1b+ hardening: resolving a digest in `prepare()` and then executing by
        mutable tag leaves a window (prepare: tag -> image A; retag; execute:
        tag -> image B). `execute()` therefore runs by the resolved image ID and
        the same identity is recorded in provenance.
        """
        proc = subprocess.run(
            ["podman", "image", "inspect", self.image, "--format", "{{.Id}}"],
            capture_output=True, text=True,
        )
        if proc.returncode != 0:
            return None, cap(
                UNSUPPORTED,
                provenance="podman image inspect",
                reason=f"image {self.image} not present locally and pulling is forbidden",
            )
        image_id = proc.stdout.strip().splitlines()[-1].strip() if proc.stdout.strip() else ""
        if not image_id:
            return None, cap(UNKNOWN, reason="inspect returned no image id")
        digest_proc = subprocess.run(
            ["podman", "image", "inspect", self.image, "--format", "{{.Digest}}"],
            capture_output=True, text=True,
        )
        digest = digest_proc.stdout.strip().splitlines()[-1].strip() if digest_proc.stdout.strip() else ""
        return image_id, cap(
            SUPPORTED,
            provenance=f"{self.image} (id={image_id[:19]}…, manifest={digest[:19]}…)",
            image_id=image_id,
            manifest_digest=digest,
        )

    def prepare(self) -> tuple[float, dict]:
        t0 = time.monotonic()
        self.ws.materialize()
        digest, obs = self.resolve_image()
        self._image_digest = digest
        self._image_id = digest
        self._image_obs = obs
        dt = (time.monotonic() - t0) * 1000.0
        return dt, {
            "image": self.image,
            "image_id": digest,
            "image_status": obs["status"],
            "image_provenance": obs.get("provenance"),
            "staging": "tar-stream-in/out (no bind mounts, no host relabel)",
            "source_ro": str(self.ws.source_ro),
            "work_rw": str(self.ws.work_rw),
            "artifacts_rw": str(self.ws.artifacts_rw),
            "tmp_ephemeral": str(self.ws.tmp_ephemeral),
        }

    # Container-side workspace. Declarative, mapped by this backend only.
    C_IN = "/s0/source"
    C_WORK = "/s0/work"
    C_ART = "/s0/artifacts"
    C_TMP = "/s0/tmp"
    C_IN_TAR = "/s0/in.tar"
    ART_MARK = "__S0_ARTIFACTS_B64__"

    def container_env(self) -> dict[str, str]:
        return {
            "S0_SOURCE": self.C_IN,
            "S0_WORK": self.C_WORK,
            "S0_ARTIFACTS": self.C_ART,
            "S0_TMP": self.C_TMP,
            "S0_MARKER": f"{self.C_WORK}/child-marker.txt",
        }

    def execute(self, scenario: dict, env: dict[str, str]) -> tuple[float, list[dict]]:
        import base64
        import shlex
        import tarfile
        import uuid

        name = f"s0run-{uuid.uuid4().hex[:12]}"
        cerr = str(scenario.get("timeout_s", 300))

        # Step 1: pack the read-only source as a tar stream (copy-in). No bind mount.
        src_bytes = b""
        if any(self.ws.source_ro.iterdir()):
            buf = io.BytesIO()
            with tarfile.open(fileobj=buf, mode="w") as tf:
                tf.add(str(self.ws.source_ro), arcname=".")
            src_bytes = buf.getvalue()

        lines = [
            "set +e",
            f"mkdir -p {self.C_WORK} {self.C_ART} {self.C_TMP} {self.C_IN}",
            f"cat > {self.C_IN_TAR}",
            f"tar xf {self.C_IN_TAR} -C {self.C_IN} 2>/dev/null || true",
            "rm -f %s" % self.C_IN_TAR,
            f"cd {self.C_WORK}",
        ]
        for step in scenario["steps"]:
            # NOTE: no host expansion here on purpose. Variables are expanded
            # inside the container shell from container_env(), so the same
            # scenario text addresses host paths (host/bwrap) or container paths
            # (podman) without the scenario knowing the difference.
            cmd = " ".join(shlex.quote(a) for a in step["run"])
            sid = step["id"]
            lines.append(f'echo "__S0_BEGIN__{sid}"')
            lines.append(cmd)
            lines.append(f'echo "__S0_END__{sid}:$?"')
        lines.append(f"tar cf - -C {self.C_ART} . 2>/dev/null | base64 -w0")
        lines.append(f'echo ""')
        lines.append(f'echo "{self.ART_MARK}"')
        script = "\n".join(lines)

        podman_cmd = [
            "podman", "run", "--rm", "--pull=never",
            "--name", name,
            "--network=none",
            "--read-only",
            "--tmpfs", "/s0:rw,size=64m",
            "-i",
            "-w", self.C_WORK,
        ]
        for k, v in self.container_env().items():
            podman_cmd += ["-e", f"{k}={v}"]
        # Run by local name with --pull=never: the resolved digest above is the
        # manifest digest for provenance, while `podman run` wants the local
        # image reference. --pull=never guarantees no registry contact.
        # Execute by the IMMUTABLE id resolved in prepare(), never by tag.
        podman_cmd += [self._image_id or self.image, "/bin/sh", "-c", script]

        started = time.monotonic()
        proc = subprocess.run(
            podman_cmd, input=src_bytes, capture_output=True, timeout=int(cerr),
        )
        elapsed = (time.monotonic() - started) * 1000.0

        out = proc.stdout.decode(errors="replace")
        errst = proc.stderr.decode(errors="replace")

        if self.ART_MARK not in out and "__S0_BEGIN__" not in out:
            return elapsed, [{
                "step": "<container>", "exit": proc.returncode,
                "elapsed_ms": round(elapsed, 2), "stdout": out, "stderr": errst,
                "errors": [f"container failed: {errst.strip().splitlines()[-1] if errst.strip() else proc.returncode}"],
                "executed_as": ["podman"],
            }]

        body, _, art_b64 = out.partition(self.ART_MARK)
        # Copy-out: extract the artifact stream on the host side.
        try:
            raw = base64.b64decode(art_b64.strip() or b"", validate=False)
            if raw:
                buf = io.BytesIO(raw)
                with tarfile.open(fileobj=buf, mode="r") as tf:
                    tf.extractall(str(self.ws.artifacts_rw))
        except Exception as exc:  # noqa: BLE001
            return elapsed, [{
                "step": "<artifacts>", "exit": None, "elapsed_ms": round(elapsed, 2),
                "stdout": "", "stderr": "", "executed_as": ["podman"],
                "errors": [f"artifact copy-out failed: {exc}"],
            }]

        results = []
        for step in scenario["steps"]:
            sid = step["id"]
            begin = f"__S0_BEGIN__{sid}\n"
            end = f"__S0_END__{sid}:"
            try:
                seg = body.split(begin, 1)[1]
                seg, tail = seg.split(end, 1)
                rc = int(tail.split("\n", 1)[0].strip())
            except (IndexError, ValueError) as exc:
                results.append({
                    "step": sid, "exit": None, "elapsed_ms": 0.0, "stdout": "", "stderr": "",
                    "errors": [f"cannot parse container output for step {sid}: {exc}"],
                    "executed_as": ["podman"],
                })
                continue
            errs = []
            for needle in step.get("expect_stdout_contains", []):
                if needle not in seg:
                    errs.append(f"stdout missing {needle!r}")
            expected_exit = step.get("expect_exit", 0)
            if rc != expected_exit:
                errs.append(f"exit {rc} != {expected_exit}")
            results.append({
                "step": sid, "exit": rc, "elapsed_ms": 0.0, "stdout": seg, "stderr": "",
                "errors": errs, "executed_as": ["podman"],
            })
        return elapsed, results

    def remaining_processes(self) -> dict:
        ps = subprocess.run(
            ["podman", "ps", "-a", "--filter", f"name={getattr(self, '_run_name', 's0run-')}",
             "--format", "{{.Names}}"],
            capture_output=True, text=True,
        )
        lines = [l for l in ps.stdout.splitlines() if l.strip()]
        return observation(SUPPORTED, value=len(lines), method="podman ps -a --filter name")

    def destroy(self, cleanup_paths: list[str]) -> tuple[float, list[str]]:
        """Verify cleanup: no container with our name may survive `--rm`."""
        t0 = time.monotonic()
        leftover = []
        ps = subprocess.run(
            ["podman", "ps", "-a", "--format", "{{.Names}}"],
            capture_output=True, text=True,
        )
        stray = [l for l in ps.stdout.splitlines() if l.startswith("s0run-")]
        if stray:
            subprocess.run(["podman", "rm", "-f", *stray], capture_output=True, text=True)
            leftover.extend(stray)
        dt = (time.monotonic() - t0) * 1000.0
        return dt, leftover




class QemuAdapter(HostAdapter):
    """QEMU/KVM: KERNEL class. Same scenario, same assertions, same schema.

    S0.1c gate rules:

    * The accelerator must be KVM. Without a usable /dev/kvm the environment is
      `unsupported`; there is NO silent TCG fallback (TCG would be a different
      placement and is not evaluated here).
    * Base is READ-ONLY: the initramfs file is never written during a run. The
      ephemeral overlay is the guest's own ramfs, discarded at poweroff.
    * No host shares: the scenario is copied INTO the initramfs at prepare time
      and artifacts come back over the serial console. There is no virtio share
      of the host filesystem.
    * Boot readiness is the guest's own `__S0_READY__` marker, never a sleep.
    * Cleanup is verified: no QEMU process survives and the per-run initramfs is
      removed.
    """

    name = "qemu"

    C_WORK = "/s0/work"
    C_ART = "/s0/artifacts"
    C_TMP = "/s0/tmp"

    def container_env(self) -> dict[str, str]:
        return {
            "S0_SOURCE": "/s0/source",
            "S0_WORK": self.C_WORK,
            "S0_ARTIFACTS": self.C_ART,
            "S0_TMP": self.C_TMP,
            "S0_MARKER": f"{self.C_WORK}/child-marker.txt",
        }

    def capabilities(self) -> dict:
        caps = super().capabilities()
        caps.update({
            "pid_isolation": cap(SUPPORTED, provenance="guest kernel"),
            "fs_isolation": cap(SUPPORTED, provenance="read-only initramfs + guest ramfs"),
            "net_isolation": cap(SUPPORTED, provenance="no NIC attached"),
            "kernel_class": cap(SUPPORTED, provenance="ephemeral guest kernel via KVM"),
            "ebpf": cap(UNKNOWN, reason="not-probed inside the guest (S0.7)"),
        })
        return caps

    def prepare(self) -> tuple[float, dict]:
        t0 = time.monotonic()
        self.ws.materialize()
        self._scenario_script = self._render_scenario()

        self._qemu = qa.qemu_binary()
        self._kernel = qa.host_kernel()
        self._kvm = qa.kvm_available()
        self._qemu_version = qa.qemu_version(self._qemu) if self._qemu else "unavailable"
        self._kernel_sha = qa._sha256_file(self._kernel) if self._kernel else ""

        self._initrd = None
        self._initrd_sha = ""
        note = ""
        if not self._kvm:
            note = "KVM accelerator unavailable; refusing to fall back to TCG"
        elif not self._qemu:
            note = "no qemu binary on this host"
        elif not self._kernel:
            note = "no host kernel image found under /usr/lib/modules/*/vmlinuz"
        if self._qemu and self._kernel and self._kvm:
            cache = Path(os.environ.get("S0_QEMU_CACHE", Path.home() / ".cache/s0run-qemu"))
            img = Path(self.ws.tmp_ephemeral).parent / "initramfs.cpio.gz"
            self._initrd, self._initrd_sha, note = qa.build_base_initramfs(
                os.environ.get("S0_QEMU_IMAGE", "docker.io/library/alpine:3.20"),
                cache,
                scenario_script=self._scenario_script,
            )

        dt = (time.monotonic() - t0) * 1000.0
        return dt, {
            "accelerator": "kvm" if self._kvm else "unsupported",
            "qemu": self._qemu_version,
            "kernel": str(self._kernel) if self._kernel else None,
            "kernel_sha256": self._kernel_sha,
            "initramfs_sha256": self._initrd_sha,
            "initramfs": str(self._initrd) if self._initrd else None,
            "staging": "scenario copied into initramfs; artifacts over serial",
            "note": note,
            "source_ro": str(self.ws.source_ro),
            "work_rw": str(self.ws.work_rw),
            "artifacts_rw": str(self.ws.artifacts_rw),
            "tmp_ephemeral": str(self.ws.tmp_ephemeral),
        }

    def _render_scenario(self) -> str:
        import shlex
        # The guest has no host environment, so the S0_* contract is exported
        # explicitly: the same scenario text addresses guest paths without
        # knowing it is running in a VM.
        exports = "\n".join(f"export {k}={shlex.quote(v)}" for k, v in self.container_env().items())
        lines = [
            "mkdir -p /s0/work /s0/artifacts /s0/tmp /s0/source",
            exports,
            "cd /s0/work",
            "set +e",
        ]
        for step in self._scenario_steps:
            cmd = " ".join(shlex.quote(a) for a in step["run"])
            lines.append(f'echo "{qa.BEGIN_MARK}{step["id"]}"')
            lines.append(cmd)
            lines.append(f'echo "{qa.END_MARK}{step["id"]}:$?"')
        return "\n".join(lines) + "\n"

    def execute(self, scenario: dict, env: dict[str, str]) -> tuple[float, list[dict]]:
        import base64
        import tarfile

        # prepare() already rendered the scenario into the initramfs; do not
        # rebuild here. A missing asset is a hard error, not a retry.
        if not self._qemu or not self._kernel or not self._kvm or not self._initrd:
            return 0.0, [{
                "step": "<boot>", "exit": None, "elapsed_ms": 0.0, "stdout": "", "stderr": "",
                "executed_as": ["qemu"],
                "errors": ["KVM accelerator unavailable; no TCG fallback by design"],
            }]

        cmd = [
            self._qemu, "-enable-kvm", "-m", os.environ.get("S0_QEMU_MEM", "512"),
            "-nographic", "-no-reboot",
            "-kernel", str(self._kernel), "-initrd", str(self._initrd),
            "-append", f"console=ttyS0 rdinit=/init panic=-1",
        ]
        started = time.monotonic()
        proc = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, text=True)
        self._proc = proc

        ready_timeout = float(os.environ.get("S0_QEMU_READY_TIMEOUT", "60"))
        out_lines: list[str] = []
        ready = False
        saw_artifacts = False
        try:
            for line in proc.stdout:  # streaming: readiness is a marker, not a sleep
                out_lines.append(line)
                if qa.READY_MARK in line:
                    ready = True
                    self._boot_ms = (time.monotonic() - started) * 1000.0
                if qa.ART_MARK in line:
                    saw_artifacts = True
                if ready and saw_artifacts:
                    break
                if (time.monotonic() - started) > ready_timeout:
                    proc.kill()
                    break
        finally:
            try:
                proc.wait(timeout=15)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()

        elapsed = (time.monotonic() - started) * 1000.0
        out = "".join(out_lines)

        if not ready:
            return elapsed, [{
                "step": "<boot>", "exit": None, "elapsed_ms": round(elapsed, 2),
                "stdout": out[-2000:], "stderr": "", "executed_as": ["qemu"],
                "errors": ["guest never reported readiness; no results accepted"],
            }]

        body, _, art_b64 = out.partition(qa.ART_MARK)
        try:
            raw = base64.b64decode(art_b64.strip().splitlines()[0] if art_b64.strip() else b"")
            if raw:
                with tarfile.open(fileobj=io.BytesIO(raw), mode="r") as tf:
                    tf.extractall(str(self.ws.artifacts_rw), filter="fully_trusted")
        except Exception as exc:  # noqa: BLE001
            return elapsed, [{
                "step": "<artifacts>", "exit": None, "elapsed_ms": round(elapsed, 2),
                "stdout": "", "stderr": "", "executed_as": ["qemu"],
                "errors": [f"artifact copy-out failed: {exc}"],
            }]

        results = []
        for step in scenario["steps"]:
            sid = step["id"]
            begin = f"{qa.BEGIN_MARK}{sid}\n"
            end = f"{qa.END_MARK}{sid}:"
            try:
                seg = body.split(begin, 1)[1]
                seg, tail = seg.split(end, 1)
                rc = int(tail.split("\n", 1)[0].strip())
            except (IndexError, ValueError) as exc:
                results.append({
                    "step": sid, "exit": None, "elapsed_ms": 0.0, "stdout": "", "stderr": "",
                    "errors": [f"cannot parse guest output for step {sid}: {exc}"],
                    "executed_as": ["qemu"],
                })
                continue
            errs = []
            for needle in step.get("expect_stdout_contains", []):
                if needle not in seg:
                    errs.append(f"stdout missing {needle!r}")
            expected_exit = step.get("expect_exit", 0)
            if rc != expected_exit:
                errs.append(f"exit {rc} != {expected_exit}")
            results.append({
                "step": sid, "exit": rc, "elapsed_ms": 0.0, "stdout": seg, "stderr": "",
                "errors": errs, "executed_as": ["qemu"],
            })
        return elapsed, results

    def remaining_processes(self) -> dict:
        proc = getattr(self, "_proc", None)
        alive = proc is not None and proc.poll() is None
        return observation(SUPPORTED, value=1 if alive else 0, method="qemu process poll")

    def destroy(self, cleanup_paths: list[str]) -> tuple[float, list[str]]:
        t0 = time.monotonic()
        leftover: list[str] = []
        proc = getattr(self, "_proc", None)
        if proc is not None and proc.poll() is None:
            proc.kill()
            proc.wait()
            leftover.append(f"qemu pid {proc.pid} had to be killed")
        initrd = getattr(self, "_initrd", None)
        if initrd is not None:
            import shutil as _sh
            path = Path(initrd)
            # The per-run assets live in "<run-id>.cpio.gz" plus the staging dir
            # "<run-id>/". Both must be gone after destroy.
            run_dir = path.with_suffix("") if path.name.endswith(".cpio.gz") else path.parent
            if run_dir.name.startswith("run-"):
                _sh.rmtree(run_dir, ignore_errors=True)
            if path.exists():
                path.unlink()
            if path.exists() or run_dir.exists():
                leftover.append(str(path))
        dt = (time.monotonic() - t0) * 1000.0
        return dt, leftover


def run(scenario: dict, env_name: str, out_dir: Path) -> dict:
    if env_name not in ADAPTERS:
        raise SystemExit(f"unknown environment {env_name!r}; known: {list(ADAPTERS)}")
    if not ADAPTERS[env_name]():
        return {
            "schema": RESULT_SCHEMA, "environment": env_name,
            "scenario": scenario["scenario_id"], "result": "skip",
            "reason": f"{env_name} not available on this host",
            "capabilities": {}, "metrics": {}, "artifacts": [],
        }

    adapter_cls = {
        "host": HostAdapter,
        "bwrap": BwrapAdapter,
        "podman": PodmanAdapter,
        "qemu": QemuAdapter,
    }.get(env_name)
    if adapter_cls is None:
        return {
            "schema": RESULT_SCHEMA, "environment": env_name,
            "scenario": scenario["scenario_id"], "result": "skip",
            "reason": f"adapter for {env_name} not implemented",
            "capabilities": {}, "metrics": {}, "artifacts": [],
        }

    ws = Workspace(out_dir / env_name)
    inst = adapter_cls(ws)
    env = ws.env()
    inst.configure(scenario)
    prepare_ms, prepared = inst.prepare()
    caps = inst.capabilities()
    exec_ms, steps = inst.execute(scenario, env)
    cleanup_paths = [expand(p, env) for p in scenario.get("cleanup_required", [])]
    cleanup_ms, leftover = inst.destroy(cleanup_paths)
    errors = [f"{s['step']}: {e}" for s in steps for e in s["errors"]]
    artifacts = [
        {"kind": "stdout_capture", "step": s["step"], "exit": s["exit"], "stdout": s["stdout"]}
        for s in steps
    ]
    return {
        "schema": RESULT_SCHEMA,
        "environment": env_name,
        "scenario": scenario["scenario_id"],
        "result": "pass" if not errors else "fail",
        "capabilities": caps,
        "metrics": {
            "prepare_ms": round(prepare_ms, 2),
            "execution_ms": round(exec_ms, 2),
            "cleanup_ms": round(cleanup_ms, 2),
            "remaining_processes": inst.remaining_processes(),
            "leftover_paths": leftover,
            "steps_run": len(steps),
        },
        "artifacts": artifacts,
        "prepared": prepared,
        "errors": errors,
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="s0run")
    ap.add_argument("--scenario", required=False, help="path to scenario JSON")
    ap.add_argument("--env", default="host", help="environment adapter (host|bwrap|podman|qemu)")
    ap.add_argument("--out", help="write JSON result to this file too")
    ap.add_argument("--list-envs", action="store_true")
    ap.add_argument("--rounds", type=int, default=1, help="repeat for reproducibility")
    args = ap.parse_args(argv)

    if args.list_envs:
        for name, probe in ADAPTERS.items():
            print(f"{name}: {'available' if probe() else 'unavailable'}")
        return 0

    if not args.scenario:
        ap.error("--scenario is required")

    scenario = load_scenario(Path(args.scenario))
    out_dir = Path(tempfile.mkdtemp(prefix=f"s0run-{args.env}-"))
    results = []
    for i in range(max(1, args.rounds)):
        r = run(scenario, args.env, out_dir / f"round{i}")
        r["round"] = i
        results.append(r)

    payload = results[0] if len(results) == 1 else {"schema": RESULT_SCHEMA, "rounds": results}
    text = json.dumps(payload, indent=2)
    print(text)
    if args.out:
        Path(args.out).write_text(text)
    return 0 if all(r.get("result") in ("pass", "skip") for r in results) else 1


if __name__ == "__main__":
    sys.exit(main())
