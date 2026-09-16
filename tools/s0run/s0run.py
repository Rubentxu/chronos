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


ADAPTERS = {
    "host": lambda: True,
    "bwrap": lambda: which("bwrap") is not None,
    "podman": lambda: which("podman") is not None,
    "qemu": lambda: which("qemu-kvm") is not None or which("qemu-system-x86_64") is not None,
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

    adapter_cls = {"host": HostAdapter, "bwrap": BwrapAdapter}.get(env_name)
    if adapter_cls is None:
        return {
            "schema": RESULT_SCHEMA, "environment": env_name,
            "scenario": scenario["scenario_id"], "result": "skip",
            "reason": f"adapter for {env_name} not implemented yet (S0.1b/c/d)",
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
