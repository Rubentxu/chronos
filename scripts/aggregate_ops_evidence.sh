#!/usr/bin/env bash
#
# aggregate_ops_evidence.sh — OPS.3 evidence aggregator.
#
# Why this exists
# ---------------
# ADR-0032 §2.2 (OPS.3): per-check, per-profile evidence files must be
# generated and aggregatable. This script reads the 16 evidence files
# (evidence/ops/ops.{1..8}.{local-stdio,linux-privileged}.json) and
# produces a single aggregate report `evidence/ops/aggregate.json` that
# summarizes the OPS.1..OPS.8 status across profiles.
#
# Usage
# -----
#   ./scripts/aggregate_ops_evidence.sh [output_path]
#
#   output_path   default: evidence/ops/aggregate.json
#
# Exit codes
# ----------
#   0 — aggregate generated successfully
#   1 — python3 not available
#   2 — at least one evidence file missing or invalid JSON
#   3 — output_path parent directory not creatable

set -euo pipefail

OUTPUT_PATH="${1:-evidence/ops/aggregate.json}"

if ! command -v python3 >/dev/null 2>&1; then
    echo "ERROR: python3 not found in PATH" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

OUTPUT_DIR="$(dirname "$OUTPUT_PATH")"
if ! mkdir -p "$OUTPUT_DIR"; then
    echo "ERROR: cannot create output directory '$OUTPUT_DIR'" >&2
    exit 3
fi

PROFILES=(local-stdio linux-privileged)
CHECKS=(OPS.1 OPS.2 OPS.3 OPS.4 OPS.5 OPS.6 OPS.7 OPS.8)

# Validate all 16 files exist + parse as JSON before aggregating.
echo "Validating 16 evidence files..."
MISSING=0
INVALID=0
for profile in "${PROFILES[@]}"; do
    for check in "${CHECKS[@]}"; do
        num="${check#OPS.}"
        file="evidence/ops/ops.${num}.${profile}.json"
        if [[ ! -f "$file" ]]; then
            echo "  MISSING: $file" >&2
            MISSING=$((MISSING + 1))
            continue
        fi
        if ! python3 -m json.tool "$file" > /dev/null 2>&1; then
            echo "  INVALID JSON: $file" >&2
            INVALID=$((INVALID + 1))
        fi
    done
done

if [[ $MISSING -gt 0 || $INVALID -gt 0 ]]; then
    echo "ERROR: $MISSING missing + $INVALID invalid evidence files" >&2
    exit 2
fi
echo "  All 16 files valid."

# Aggregate.
TIMESTAMP="$(python3 -c 'import time; print(int(time.time()))')"
REPO_ROOT="$REPO_ROOT" \
OUTPUT_PATH="$OUTPUT_PATH" \
TIMESTAMP="$TIMESTAMP" \
python3 <<'PYEOF'
import json
import os
from pathlib import Path

repo_root = Path(os.environ["REPO_ROOT"])
output_path = Path(os.environ["OUTPUT_PATH"])
timestamp = int(os.environ["TIMESTAMP"])

profiles = ["local-stdio", "linux-privileged"]
checks = ["OPS.1", "OPS.2", "OPS.3", "OPS.4", "OPS.5", "OPS.6", "OPS.7", "OPS.8"]

per_check = {}
for check in checks:
    num = check.split(".")[1]
    per_check[check] = {}
    for profile in profiles:
        path = repo_root / "evidence" / "ops" / f"ops.{num}.{profile}.json"
        with open(path, "r", encoding="utf-8") as f:
            doc = json.load(f)
        per_check[check][profile] = {
            "status": doc["status"],
            "gaps_count": len(doc.get("gaps", [])),
            "evidence_count": len(doc.get("evidence", [])),
        }

# Per-profile summary.
per_profile = {}
for profile in profiles:
    statuses = [per_check[c][profile]["status"] for c in checks]
    per_profile[profile] = {
        "pass": sum(1 for s in statuses if s == "pass"),
        "warn": sum(1 for s in statuses if s == "warn"),
        "fail": sum(1 for s in statuses if s == "fail"),
        "total": len(statuses),
    }

# Overall tier per profile (per OPS.2 tier rules):
#   - All pass (no warn, no fail) → cert-4-production
#   - No fail + some warn → cert-3-certified
#   - 1-2 fail → cert-2-partial
#   - 3+ fail → cert-1-stub
def tier_from_summary(summary):
    if summary["fail"] == 0 and summary["warn"] == 0:
        return "cert-4-production"
    if summary["fail"] == 0:
        return "cert-3-certified"
    if summary["fail"] <= 2:
        return "cert-2-partial"
    return "cert-1-stub"

aggregate = {
    "schema_version": 1,
    "per_check": per_check,
    "per_profile": per_profile,
    "tier_per_profile": {p: tier_from_summary(per_profile[p]) for p in profiles},
    "generated_at_unix_seconds": timestamp,
    "source_count": len(profiles) * len(checks),
}

output_path.parent.mkdir(parents=True, exist_ok=True)
with open(output_path, "w", encoding="utf-8") as f:
    json.dump(aggregate, f, indent=2, sort_keys=True, ensure_ascii=False)
    f.write("\n")

print(f"OPS.3 aggregate written to: {output_path}")
for profile in profiles:
    s = per_profile[profile]
    t = aggregate["tier_per_profile"][profile]
    print(f"  profile={profile} tier={t} ({s['pass']}/{s['total']} pass, {s['warn']} warn, {s['fail']} fail)")
PYEOF

echo "Aggregate generated."
exit 0
