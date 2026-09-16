#!/usr/bin/env python3
"""SANDBOX-S0.2 negative cases: the confusions the execution contracts prevent.

Each case asserts the WRONG answer is impossible, not merely that the right one
is present.
"""
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SC = ROOT / "scenarios" / "child-process-lifecycle.json"
fail = []


def run(env, extra_env=None):
    import os
    e = {**os.environ, **(extra_env or {})}
    p = subprocess.run(
        [sys.executable, str(ROOT / "s0run.py"), "--scenario", str(SC), "--env", env],
        capture_output=True, text=True, env=e, cwd=str(ROOT),
    )
    return json.loads(p.stdout)


def check(name, cond, detail=""):
    print(f"  {'PASS' if cond else 'FAIL'}: {name}{(' — ' + detail) if detail else ''}")
    if not cond:
        fail.append(name)


# 1. unsupported != fail, and an unavailable env never runs on the host
r = run("podman", {"S0_FORCE_UNAVAILABLE": "podman"})
check("unavailable env is 'skip'/'unsupported', not 'fail'", r["result"] == "skip", r["result"])
check("a skipped env reports no metrics", r["metrics"] == {})

# 2. not measured != supported (capabilities are tri-state)
r = run("host")
caps = r["capabilities"]
check("unmeasured capability is 'unknown', not 'supported'",
      caps["ebpf"]["status"] == "unknown", json.dumps(caps["ebpf"]))
check("a probed capability carries provenance",
      "provenance" in caps["ptrace"] or caps["ptrace"]["status"] == "unknown",
      json.dumps(caps["ptrace"]))

# 3. unknown != false (remaining_processes on host is not a fabricated zero)
rp = r["metrics"]["remaining_processes"]
check("unattributable metric is 'unknown' with method, not 0",
      rp["status"] == "unknown" and rp["value"] is None and "method" in rp,
      json.dumps(rp))

# 4. a broken environment fails loudly and never leaks another placement's output
r = run("podman", {"S0_PODMAN_IMAGE": "docker.io/library/does-not-exist-s0:0"})
check("broken environment is 'fail' with an error", r["result"] == "fail" and r["errors"])
check("no host output leaks into a failed container run",
      not any("parent-start" in (a.get("stdout") or "") for a in r["artifacts"]))

# 5. cleanup unverified != cleanup success (leftovers are reported)
check("cleanup leftovers are reported, not assumed clean",
      isinstance(r["metrics"]["leftover_paths"], list))

# 6. schemas are the chronos.execution.* ones
check("result carries the chronos.execution namespace",
      r["schema"] == "chronos.execution.result/v1", r["schema"])

# 7. the scenario schema is enforced, not guessed
bad = ROOT / "schemas" / "_tmp_bad_scenario.json"
bad.write_text(json.dumps({"schema": "sddk.sandbox.scenario/v1", "scenario_id": "x", "steps": []}))
p = subprocess.run([sys.executable, str(ROOT / "s0run.py"), "--scenario", str(bad), "--env", "host"],
                   capture_output=True, text=True, cwd=str(ROOT))
check("a foreign scenario schema is rejected", p.returncode != 0 and "unsupported scenario schema" in p.stderr)
bad.unlink()

print("CONTRACT_CASES=" + ("PASS" if not fail else f"FAIL ({len(fail)})"))
sys.exit(1 if fail else 0)
