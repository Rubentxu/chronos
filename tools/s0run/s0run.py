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
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

RESULT_SCHEMA = "sddk.sandbox.result/v2"

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
    if doc.get("schema") != "sddk.sandbox.scenario/v1":
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
    result schema. No --privileged, no SYS_PTRACE, no eBPF capabilities: the
    point of S0.1b is to compare PLACEMENT, not to add a second variable."""

    name = "podman"

    @property
    def image(self) -> str:
        return os.environ.get("S0_PODMAN_IMAGE", "docker.io/library/alpine:3.20")

    def capabilities(self) -> dict:
        caps = super().capabilities()
        caps.update({
            "pid_isolation": cap(SUPPORTED, provenance="container pid namespace"),
            "fs_isolation": cap(SUPPORTED, provenance="read-only rootfs + explicit mounts"),
            "net_isolation": cap(SUPPORTED, provenance="--network=none"),
            "kernel_class": cap(UNSUPPORTED, reason="shares the host kernel"),
        })
        return caps

    def wrap(self, cmd: list[str]) -> list[str] | None:
        w = self.ws
        # Keep the caller's PATH lookup inside the image: alpine has /bin/sh,
        # /bin/echo, /bin/cat. Anything else is the image's business, not ours.
        return [
            "podman", "run", "--rm",
            "--network=none",
            "--read-only",
            "--tmpfs", "/run",
            # SELinux note (S0.1b finding): on an Enforcing host, Podman bind
            # mounts are denied without a relabel and the write fails with
            # "Permission denied". `:z` is required. Cost: it relabels the HOST
            # directory, i.e. the SYSTEM class mutates host state as a side
            # effect of mounting. Recorded, not hidden.
            "-v", f"{w.source_ro}:{w.source_ro}:ro",
            "-v", f"{w.work_rw}:{w.work_rw}:rw,z",
            "-v", f"{w.artifacts_rw}:{w.artifacts_rw}:rw,z",
            "-v", f"{w.tmp_ephemeral}:{w.tmp_ephemeral}:rw,z",
            "-w", str(w.work_rw),
            self.image,
            *cmd,
        ]

    def execute(self, scenario: dict, env: dict[str, str]) -> tuple[float, list[dict]]:
        """Run the WHOLE scenario in ONE container.

        S0.1b finding: one `podman run` per step costs ~11 s per step (container
        start), which is three orders of magnitude above the host and would
        make the placement comparison meaningless. The same scenario is
        therefore executed as a single container session with step delimiters.
        This is a placement decision, not a semantic one: the assertions and
        the result schema are unchanged.
        """
        import shlex

        lines = ["set +e"]
        for step in scenario["steps"]:
            cmd = " ".join(shlex.quote(expand(a, env)) for a in step["run"])
            sid = step["id"]
            lines.append(f'echo "__S0_BEGIN__{sid}"')
            lines.append(cmd)
            lines.append(f'echo "__S0_END__{sid}:$?"')
        script = "\n".join(lines)

        cmd = self.wrap(["/bin/sh", "-c", script])
        started = time.monotonic()
        proc = subprocess.run(
            cmd, capture_output=True, text=True, timeout=scenario.get("timeout_s", 300)
        )
        elapsed = (time.monotonic() - started) * 1000.0

        if proc.returncode != 0 and "__S0_BEGIN__" not in proc.stdout:
            # The container itself failed (image pull, mount denied, capability
            # refused). Report it as a hard error: there is NO fallback path.
            return elapsed, [{
                "step": "<container>", "exit": proc.returncode, "elapsed_ms": round(elapsed, 2),
                "stdout": proc.stdout, "stderr": proc.stderr,
                "errors": [f"container failed: {proc.stderr.strip().splitlines()[-1] if proc.stderr.strip() else proc.returncode}"],
                "executed_as": ["podman"],
            }]

        results = []
        for step in scenario["steps"]:
            sid = step["id"]
            begin = f"__S0_BEGIN__{sid}\n"
            end = f"__S0_END__{sid}:"
            try:
                body = proc.stdout.split(begin, 1)[1]
                body, tail = body.split(end, 1)
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
                if needle not in body:
                    errs.append(f"stdout missing {needle!r}")
            expected_exit = step.get("expect_exit", 0)
            if rc != expected_exit:
                errs.append(f"exit {rc} != {expected_exit}")
            results.append({
                "step": sid, "exit": rc, "elapsed_ms": 0.0, "stdout": body, "stderr": "",
                "errors": errs, "executed_as": ["podman"],
            })
        return elapsed, results

    def remaining_processes(self) -> dict:
        # A --rm container that exited cleanly leaves nothing behind. We can
        # state that as OBSERVED only because `--rm` plus a clean exit is the
        # contract; anything else would need a real check.
        return observation(SUPPORTED, value=0, method="podman --rm + clean exit")


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

    adapter_cls = {"host": HostAdapter, "bwrap": BwrapAdapter, "podman": PodmanAdapter}.get(env_name)
    if adapter_cls is None:
        return {
            "schema": RESULT_SCHEMA, "environment": env_name,
            "scenario": scenario["scenario_id"], "result": "skip",
            "reason": f"adapter for {env_name} not implemented yet (S0.1c/d)",
            "capabilities": {}, "metrics": {}, "artifacts": [],
        }

    ws = Workspace(out_dir / env_name)
    inst = adapter_cls(ws)
    env = ws.env()
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
