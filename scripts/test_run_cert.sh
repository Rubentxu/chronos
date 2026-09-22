#!/usr/bin/env bash
#
# test_run_cert.sh — OPS.2 executable certification tier runner test.
#
# Why this exists
# ---------------
# ADR-0032 §2.2 (OPS.2): the executable certification tier runner
# (`scripts/run_cert.sh`) must have at least 1 shell test asserting
# deterministic output + valid schema. This is that test.
#
# Usage
# -----
#   ./scripts/test_run_cert.sh
#
# Exit codes
# ----------
#   0 — all assertions pass
#   1 — at least one assertion failed
#
# This test does NOT exercise the full binary cert path (that's M11.4
# territory). It exercises the structural output: the cert-report.json
# is generated, parses as valid JSON, and contains the expected schema.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

if ! command -v python3 >/dev/null 2>&1; then
    echo "ERROR: python3 not found in PATH" >&2
    exit 1
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

REPORT_LOCAL="$TMP_DIR/cert-local.json"
REPORT_LINUX="$TMP_DIR/cert-linux.json"

echo "Test 1: invalid profile → exit 1"
if bash scripts/run_cert.sh invalid-profile 2>/dev/null; then
    echo "FAIL: invalid profile did not exit non-zero"
    exit 1
fi
echo "  PASS"

echo "Test 2: local-stdio profile → exit 0 + valid JSON"
if ! bash scripts/run_cert.sh local-stdio "$REPORT_LOCAL"; then
    echo "FAIL: local-stdio did not exit 0"
    exit 1
fi
if ! python3 -m json.tool "$REPORT_LOCAL" > /dev/null; then
    echo "FAIL: local-stdio JSON is invalid"
    exit 1
fi
echo "  PASS"

echo "Test 3: linux-privileged profile → exit 0 + valid JSON"
if ! bash scripts/run_cert.sh linux-privileged "$REPORT_LINUX"; then
    echo "FAIL: linux-privileged did not exit 0"
    exit 1
fi
if ! python3 -m json.tool "$REPORT_LINUX" > /dev/null; then
    echo "FAIL: linux-privileged JSON is invalid"
    exit 1
fi
echo "  PASS"

echo "Test 4: reports have expected schema fields"
python3 - <<PYEOF
import json
import sys

with open("$REPORT_LOCAL", "r", encoding="utf-8") as f:
    local = json.load(f)
with open("$REPORT_LINUX", "r", encoding="utf-8") as f:
    linux = json.load(f)

required_fields = {"schema_version", "profile", "commit_sha", "tier", "checks", "summary", "downgrade_reasons", "generated_at_unix_seconds"}
for label, report in [("local", local), ("linux", linux)]:
    missing = required_fields - set(report.keys())
    if missing:
        print(f"FAIL: {label} report missing fields: {missing}", file=sys.stderr)
        sys.exit(1)
    if report["schema_version"] != 1:
        print(f"FAIL: {label} schema_version is {report['schema_version']}, expected 1", file=sys.stderr)
        sys.exit(1)
    if report["profile"] not in ("local-stdio", "linux-privileged"):
        print(f"FAIL: {label} profile is {report['profile']}", file=sys.stderr)
        sys.exit(1)
    if report["tier"] not in ("cert-1-stub", "cert-2-partial", "cert-3-certified", "cert-4-production"):
        print(f"FAIL: {label} tier is {report['tier']}", file=sys.stderr)
        sys.exit(1)
    if not isinstance(report["checks"], list) or len(report["checks"]) < 8:
        print(f"FAIL: {label} checks list has {len(report['checks'])} items, expected >= 8 (OPS.1..OPS.8)", file=sys.stderr)
        sys.exit(1)
    if not isinstance(report["generated_at_unix_seconds"], int):
        print(f"FAIL: {label} generated_at_unix_seconds is not an int", file=sys.stderr)
        sys.exit(1)

print("  PASS")
PYEOF

echo "Test 5: summary counters are non-negative and sum to checks length"
python3 - <<PYEOF
import json
import sys

for label, path in [("local", "$REPORT_LOCAL"), ("linux", "$REPORT_LINUX")]:
    with open(path, "r", encoding="utf-8") as f:
        report = json.load(f)
    s = report["summary"]
    if s["pass"] < 0 or s["warn"] < 0 or s["fail"] < 0:
        print(f"FAIL: {label} summary has negative counters", file=sys.stderr)
        sys.exit(1)
    if s["pass"] + s["warn"] + s["fail"] != s["total"]:
        print(f"FAIL: {label} summary counters don't sum to total", file=sys.stderr)
        sys.exit(1)
    if s["total"] != len(report["checks"]):
        print(f"FAIL: {label} summary.total != len(checks)", file=sys.stderr)
        sys.exit(1)

print("  PASS")
PYEOF

echo ""
echo "All 5 tests passed."
exit 0
