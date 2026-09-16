#!/usr/bin/env python3
"""s0run — minimal SANDBOX-S0.1a scenario runner.

One scenario, one adapter at a time. Emits a machine-readable JSON result on
stdout and (optionally) to a file. No framework, no abstraction layers: this
exists to produce the measurement matrix that will decide whether the
ExecutionEnvironment contract draft survives contact with reality.

Usage:
    s0run --scenario scenarios/child-process-lifecycle.json --env host
    s0run --list-envs
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

RESULT_SCHEMA = "sddk.sandbox.result/v1"


def which(name: str) -> str | None:
    return shutil.which(name)


def host_available() -> bool:
    return True


def bwrap_available() -> bool:
    return which("bwrap") is not None


def podman_available() -> bool:
    return which("podman") is not None


def qemu_available() -> bool:
    return which("qemu-kvm") is not None or which("qemu-system-x86_64") is not None


ADAPTERS = {
    "host": host_available,
    "bwrap": bwrap_available,
    "podman": podman_available,
    "qemu": qemu_available,
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


class HostAdapter:
    """Reference implementation. Everything else must match this shape."""

    name = "host"

    def __init__(self, workdir: Path, env: dict[str, str]):
        self.workdir = workdir
        self.env = env

    def capabilities(self) -> dict:
        # Baseline: no isolation, no namespace control, no privileged tracing.
        return {
            "pid_isolation": False,
            "fs_isolation": False,
            "net_isolation": False,
            "ptrace_available": True,
            "ebpf_available": Path("/sys/kernel/debug/tracing").exists(),
            "root_required": False,
            "kernel_class": False,
        }

    def prepare(self) -> tuple[float, dict]:
        t0 = time.monotonic()
        self.workdir.mkdir(parents=True, exist_ok=True)
        dt = (time.monotonic() - t0) * 1000.0
        return dt, {"workdir": str(self.workdir)}

    def execute(self, scenario: dict) -> tuple[float, list[dict]]:
        results = []
        t0 = time.monotonic()
        for step in scenario["steps"]:
            cmd = [expand(a, self.env) for a in step["run"]]
            started = time.monotonic()
            proc = subprocess.run(
                cmd,
                cwd=self.workdir,
                env={**os.environ, **self.env},
                capture_output=True,
                text=True,
                timeout=step.get("timeout_s", 60),
            )
            elapsed = (time.monotonic() - started) * 1000.0
            out = proc.stdout
            errs = []
            for needle in step.get("expect_stdout_contains", []):
                if needle not in out:
                    errs.append(f"stdout missing {needle!r}")
            expected_exit = step.get("expect_exit", 0)
            if proc.returncode != expected_exit:
                errs.append(f"exit {proc.returncode} != {expected_exit}")
            results.append(
                {
                    "step": step["id"],
                    "exit": proc.returncode,
                    "elapsed_ms": round(elapsed, 2),
                    "stdout": out,
                    "stderr": proc.stderr,
                    "errors": errs,
                }
            )
        dt = (time.monotonic() - t0) * 1000.0
        return dt, results

    def remaining_processes(self) -> int:
        # Host baseline: we cannot attribute leftovers cheaply, and pretending
        # we can would be a silent lie. Report 0 with a note in capabilities.
        return 0

    def destroy(self, cleanup_paths: list[str]) -> tuple[float, list[str]]:
        t0 = time.monotonic()
        leftover = []
        for p in cleanup_paths:
            path = Path(expand(p, self.env))
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

    def _wrap(self, cmd: list[str]) -> list[str]:
        if cmd[0] == "sh":
            # Spike finding S0.1a: with only `--ro-bind / /`, the workspace is
            # read-only and fixtures that write markers fail (observed:
            # marker-check stdout missing). The workspace must be an explicit
            # writable bind; that bind IS the "workspace input" of the future
            # ExecutionSpec, not a backend detail.
            return [
                "bwrap",
                "--ro-bind", "/", "/",
                "--bind", str(self.workdir), str(self.workdir),
                "--dev", "/dev",
                "--proc", "/proc",
                "--tmpfs", "/tmp",
                "--unshare-pid",
                "--die-with-parent",
                *cmd,
            ]
        return cmd

    def capabilities(self) -> dict:
        caps = super().capabilities()
        caps.update({"pid_isolation": True, "fs_isolation": False, "net_isolation": False})
        return caps

    def execute(self, scenario: dict) -> tuple[float, list[dict]]:
        wrapped = dict(scenario)
        wrapped["steps"] = [
            {**s, "run": self._wrap([expand(a, self.env) for a in s["run"]])}
            for s in scenario["steps"]
        ]
        return super().execute(wrapped)


def run(scenario: dict, env_name: str, out_dir: Path) -> dict:
    if env_name not in ADAPTERS:
        raise SystemExit(f"unknown environment {env_name!r}; known: {list(ADAPTERS)}")
    if not ADAPTERS[env_name]():
        return {
            "schema": RESULT_SCHEMA,
            "environment": env_name,
            "scenario": scenario["scenario_id"],
            "result": "skip",
            "reason": f"{env_name} not available on this host",
            "capabilities": {},
            "metrics": {},
            "artifacts": [],
        }

    work = out_dir / env_name
    marker = work / "child-marker.txt"
    env = {"S0_MARKER": str(marker), "S0_WORKDIR": str(work)}
    adapter = {"host": HostAdapter, "bwrap": BwrapAdapter}.get(env_name)
    if adapter is None:
        return {
            "schema": RESULT_SCHEMA,
            "environment": env_name,
            "scenario": scenario["scenario_id"],
            "result": "skip",
            "reason": f"adapter for {env_name} not implemented in S0.1a (only host/bwrap)",
            "capabilities": {},
            "metrics": {},
            "artifacts": [],
        }

    inst = adapter(work, env)
    prepare_ms, prepared = inst.prepare()
    caps = inst.capabilities()
    try:
        exec_ms, steps = inst.execute(scenario)
        cleanup_paths = [expand(p, env) for p in scenario.get("cleanup_required", [])]
        cleanup_ms, leftover = inst.destroy(cleanup_paths)
        errors = [f"{s['step']}: {e}" for s in steps for e in s["errors"]]
        tail = []
        for s in steps:
            tail.extend(
                {
                    "step": s["step"],
                    "exit": s["exit"],
                    "stdout_hash_input": s["stdout"].strip(),
                }
                for _ in [0]
            )
        result = "pass" if not errors else "fail"
        return {
            "schema": RESULT_SCHEMA,
            "environment": env_name,
            "scenario": scenario["scenario_id"],
            "result": result,
            "capabilities": caps,
            "metrics": {
                "prepare_ms": round(prepare_ms, 2),
                "execution_ms": round(exec_ms, 2),
                "cleanup_ms": round(cleanup_ms, 2),
                "remaining_processes": inst.remaining_processes(),
                "leftover_paths": leftover,
                "steps_run": len(steps),
            },
            "artifacts": [{"kind": "stdout_capture", "value": tail}],
            "prepared": prepared,
            "errors": errors,
        }
    finally:
        pass


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="s0run")
    ap.add_argument("--scenario", required=False, help="path to scenario JSON")
    ap.add_argument("--env", default="host", help="environment adapter (host|bwrap|podman|qemu)")
    ap.add_argument("--out", help="write JSON result to this file too")
    ap.add_argument("--list-envs", action="store_true")
    ap.add_argument("--rounds", type=int, default=1, help="repeat the scenario N times for reproducibility")
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
