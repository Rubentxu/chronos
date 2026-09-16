#!/usr/bin/env python3
"""Compare exact deferred-test observations with the ledger-derived manifest."""
from __future__ import annotations
import argparse, json
from pathlib import Path

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--manifest", type=Path, required=True)
    ap.add_argument("--observations", type=Path, required=True)
    ap.add_argument("--report", type=Path, required=True)
    args = ap.parse_args()
    manifest = json.loads(args.manifest.read_text())
    observed_doc = json.loads(args.observations.read_text())
    declared = {test for entry in manifest["deferred"] for test in entry["tests"]}
    observations = observed_doc.get("observations", [])
    by_id = {item["qualified_id"]: item for item in observations}
    issues = []
    if "prebuild_failed" in observed_doc:
        issues.append("CONFIGURATION DRIFT: deferred target prebuild failed")
    missing = declared - set(by_id)
    extra = set(by_id) - declared
    if missing: issues.append("CONTRACT DRIFT: not executed: " + ", ".join(sorted(missing)))
    if extra: issues.append("UNCLASSIFIED_REGRESSION: undeclared execution: " + ", ".join(sorted(extra)))
    expected = {test for test, item in by_id.items() if item["result"] == "EXPECTED_FAILURE"}
    for test, item in sorted(by_id.items()):
        if item["result"] != "EXPECTED_FAILURE":
            issues.append(f"{item['result']}: {test}")
    if expected != declared:
        issues.append("EXACT_SET_MISMATCH: observed expected failures do not equal declared failures")
    verdict = "GREEN" if not issues else "RED"
    lines = ["# Sandbox Debt Sentinel", "", f"**Verdict: {verdict}**", "",
        f"- Declared deferred failures: {len(declared)}", f"- Expected failures observed: {len(expected)}", "",
        "| Qualified ID | Result | Duration ms | Exit | Output |", "|---|---|---:|---:|---|"]
    for item in observations:
        lines.append(f"| `{item['qualified_id']}` | {item['result']} | {item['duration_ms']} | {item['exit_code']} | `{item['output_path']}` |")
    lines += ["", "## Result"] + ([f"- {issue}" for issue in issues] or ["- `observed_expected_failures == declared_deferred_tests`"])
    args.report.write_text("\n".join(lines) + "\n")
    (args.report.parent / "sentinel-verdict").write_text(verdict + "\n")
    return 0 if verdict == "GREEN" else 1
if __name__ == "__main__": raise SystemExit(main())
