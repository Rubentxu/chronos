#!/usr/bin/env python3
"""Run each ledger-declared deferred integration test exactly once after prebuild."""
from __future__ import annotations
import argparse, json, subprocess, time
from pathlib import Path


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--manifest", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--timeout-seconds", type=int, default=120)
    args = ap.parse_args()
    manifest = json.loads(args.manifest.read_text())
    tests = [test for entry in manifest["deferred"] for test in entry["tests"]]
    targets = {(run["package"], run["target"]) for run in manifest["runs"]}
    args.output.parent.mkdir(parents=True, exist_ok=True)

    # Compile first. Runtime duration and timeout begin only below.
    for package, target in sorted(targets):
        cmd = ["cargo", "test", "-p", package, "--test", target, "--no-run"]
        built = subprocess.run(cmd, text=True, capture_output=True)
        if built.returncode:
            args.output.write_text(json.dumps({"version": 1, "prebuild_failed": {
                "package": package, "target": target, "exit_code": built.returncode,
                "output": built.stdout + built.stderr}}, indent=2) + "\n")
            return 2

    observations = []
    for qualified_id in tests:
        target, test_name = qualified_id.split("::", 1)
        package = next(run["package"] for run in manifest["runs"] if run["target"] == target)
        output_path = args.output.parent / f"{target}__{test_name}.log"
        cmd = ["cargo", "test", "-p", package, "--test", target, test_name,
               "--", "--exact", "--test-threads=1", "--nocapture"]
        started = time.monotonic()
        try:
            run = subprocess.run(cmd, text=True, capture_output=True, timeout=args.timeout_seconds)
            text = run.stdout + run.stderr
            output_path.write_text(text)
            exact_failed = (
                run.returncode != 0
                and "test result: FAILED" in text
                and any(line.strip() == test_name for line in text.splitlines())
            )
            exact_passed = run.returncode == 0 and "test result: ok" in text
            result = "EXPECTED_FAILURE" if exact_failed else (
                "STALE_WAIVER" if exact_passed else "UNCLASSIFIED_REGRESSION")
            observations.append({"qualified_id": qualified_id, "target": target, "rust_test_name": test_name,
                "exit_code": run.returncode, "result": result, "duration_ms": round((time.monotonic()-started)*1000),
                "timeout": False, "output_path": str(output_path)})
        except subprocess.TimeoutExpired as exc:
            output_path.write_text((exc.stdout or "") + (exc.stderr or ""))
            observations.append({"qualified_id": qualified_id, "target": target, "rust_test_name": test_name,
                "exit_code": None, "result": "EXECUTION_TIMEOUT", "duration_ms": round((time.monotonic()-started)*1000),
                "timeout": True, "output_path": str(output_path)})
    args.output.write_text(json.dumps({"version": 1, "observations": observations}, indent=2) + "\n")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
