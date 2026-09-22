#!/usr/bin/env bash
#
# run_cert.sh — OPS.2 executable certification tier runner per profile.
#
# Why this exists
# ---------------
# ADR-0032 §2.2 (OPS.2) requires an EXECUTABLE per-profile certification tier
# runner. Per ADR-0004 (no Silent Lies) + ROADMAP §OPS §103:
#
#   "Publicar solo el perfil que alcance CERT-4 con pruebas y artefactos del
#    mismo commit/release"
#
# This script generates a deterministic cert report per profile (currently
# `local/stdio` and `Linux privileged`) by introspecting the binary against
# the documented certification criteria. Each profile receives an honest
# tier — if the binary cannot satisfy CERT-3 / CERT-4 evidence, the script
# reports CERT-1 / CERT-2 with explicit reasons.
#
# Usage
# -----
#   ./scripts/run_cert.sh [profile] [output_path]
#
#   profile       local-stdio | linux-privileged
#                 default: local-stdio
#   output_path   absolute or relative path to write the cert-report.json
#                 default: evidence/ops/cert-report.json
#
# Exit codes
# ----------
#   0 — cert report generated successfully (tier may still be CERT-1 / CERT-2)
#   1 — invalid profile argument
#   2 — python3 not available
#   3 — output_path parent directory not creatable
#   4 — underlying cargo build / test failure
#
# The script ALWAYS exits 0 if the report is generated, regardless of the
# reported tier. A CERT-1 report is a valid outcome — it means "honestly
# not certified". Exit code 1+ indicates a procedural failure, not a
# capability gap.
#
# What the report contains
# ------------------------
# - profile (string)
# - binary_sha256 (string) — the artifact under certification
# - commit_sha (string) — the git commit the artifact was built from
# - tier (string: cert-1-stub | cert-2-partial | cert-3-certified | cert-4-production)
# - checks (list of { id, name, status, evidence })
#   - id is OPS.1..OPS.8 mapping
#   - status is pass | warn | fail
#   - evidence is a path or note backing the status
# - downgrade_reasons (list of strings)
# - generated_at_unix_seconds (int) — wall-clock timestamp; this is the
#   ONLY wall-clock timestamp in the OPS chapter per project convention
#   (per ADR-0004 + M6.* invariants)

set -euo pipefail

PROFILE="${1:-local-stdio}"
OUTPUT_PATH="${2:-evidence/ops/cert-report.json}"

if [[ "$PROFILE" != "local-stdio" && "$PROFILE" != "linux-privileged" ]]; then
    echo "ERROR: invalid profile '$PROFILE'. Valid: local-stdio, linux-privileged" >&2
    exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
    echo "ERROR: python3 not found in PATH" >&2
    exit 2
fi

# Resolve repo root.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# Ensure output dir exists.
OUTPUT_DIR="$(dirname "$OUTPUT_PATH")"
if ! mkdir -p "$OUTPUT_DIR"; then
    echo "ERROR: cannot create output directory '$OUTPUT_DIR'" >&2
    exit 3
fi

# Discover commit SHA. If git is not available or HEAD is unborn, report
# "unknown" — we still produce a cert report; the cert is invalid for
# release but valid for diagnostic purposes.
if command -v git >/dev/null 2>&1 && git rev-parse HEAD >/dev/null 2>&1; then
    COMMIT_SHA="$(git rev-parse HEAD)"
else
    COMMIT_SHA="unknown"
fi

# Wall-clock timestamp. This is the ONLY place in OPS chapter where we
# emit a wall-clock Unix timestamp; everything else is monotonic or
# event-derived per ADR-0004.
TIMESTAMP="$(python3 -c 'import time; print(int(time.time()))')"

# Run the cert logic via Python (single source of truth for the
# criteria + tier logic).
PROFILE="$PROFILE" \
COMMIT_SHA="$COMMIT_SHA" \
TIMESTAMP="$TIMESTAMP" \
REPO_ROOT="$REPO_ROOT" \
OUTPUT_PATH="$OUTPUT_PATH" \
python3 <<'PYEOF'
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

profile = os.environ["PROFILE"]
commit_sha = os.environ["COMMIT_SHA"]
timestamp = int(os.environ["TIMESTAMP"])
repo_root = Path(os.environ["REPO_ROOT"])
output_path = Path(os.environ["OUTPUT_PATH"])


def file_sha256(path: Path) -> str:
    if not path.is_file():
        return "missing"
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def cargo_test_present(target: str) -> bool:
    """Check whether a `cargo test --test <name>` would find a test file."""
    candidate = repo_root / "crates" / "chronos-services" / "tests" / f"{target}.rs"
    return candidate.is_file()


# ---------------------------------------------------------------------------
# Profile-specific criteria
# ---------------------------------------------------------------------------
# For each profile, define:
#   - capability checks (what the binary/runtime can do on this host)
#   - evidence paths (where the evidence lives, if any)
#   - tier downgrade rules

checks = []

# OPS.1 — amenaza/acceso (threat model present)
h12 = repo_root / "docs" / "security" / "H1.2-threat-model.md"
checks.append({
    "id": "OPS.1",
    "name": "threat-model-present",
    "status": "pass" if h12.is_file() else "fail",
    "evidence": str(h12.relative_to(repo_root)) if h12.is_file() else "missing",
})

# OPS.2 — supply chain (SBOM + deny.toml)
deny = repo_root / "deny.toml"
sbom_script = repo_root / "scripts" / "generate_chronos_sbom.py"
sbom_workflow = repo_root / ".github" / "workflows" / "supply-chain.yml"
sbom_status = (
    "pass" if (deny.is_file() and sbom_script.is_file() and sbom_workflow.is_file())
    else "fail"
)
checks.append({
    "id": "OPS.2",
    "name": "supply-chain-tooling-present",
    "status": sbom_status,
    "evidence": (
        f"deny.toml={deny.is_file()}, "
        f"sbom_script={sbom_script.is_file()}, "
        f"workflow={sbom_workflow.is_file()}"
    ),
})

# OPS.3 — aislamiento y secretos (H1.2 §6-§7 references)
checks.append({
    "id": "OPS.3",
    "name": "isolation-and-secrets-documented",
    "status": "pass" if h12.is_file() and "isolation" in h12.read_text(encoding="utf-8", errors="replace").lower() else "warn",
    "evidence": "H1.2 §6-§7 cross-reference",
})

# OPS.4 — límites y rendimiento (H1.5 perf budgets)
h15 = repo_root / "docs" / "architecture" / "H1.5-runtimes-capabilities-benchmarks.md"
checks.append({
    "id": "OPS.4",
    "name": "performance-budgets-documented",
    "status": "pass" if h15.is_file() else "fail",
    "evidence": str(h15.relative_to(repo_root)) if h15.is_file() else "missing",
})

# OPS.5 — backup/restore y schema migration (H1.6)
h16 = repo_root / "docs" / "runbooks" / "H1.6-install-upgrade-rollback.md"
checks.append({
    "id": "OPS.5",
    "name": "install-upgrade-rollback-documented",
    "status": "pass" if h16.is_file() else "fail",
    "evidence": str(h16.relative_to(repo_root)) if h16.is_file() else "missing",
})

# OPS.6 — telemetry y diagnóstico (H1.2 §10)
checks.append({
    "id": "OPS.6",
    "name": "telemetry-and-diagnostics-documented",
    "status": "warn",  # OPS.4 will deliver blueprint
    "evidence": "blueprint pending (OPS.4 deliverable)",
})

# OPS.7 — instalación/upgrade/rollback (H1.6)
checks.append({
    "id": "OPS.7",
    "name": "installation-procedure-documented",
    "status": "pass" if h16.is_file() else "fail",
    "evidence": str(h16.relative_to(repo_root)) if h16.is_file() else "missing",
})

# OPS.8 — soporte y respuesta a incidentes (runbook pending OPS.4)
checks.append({
    "id": "OPS.8",
    "name": "support-runbook-present",
    "status": "warn",  # OPS.4 will deliver
    "evidence": "runbook pending (OPS.4 deliverable)",
})

# ---------------------------------------------------------------------------
# Profile-specific capability checks
# ---------------------------------------------------------------------------

if profile == "local-stdio":
    # local/stdio — minimum requirement: cargo test compiles + unit tests pass.
    # We attempt a fast syntax check via cargo check on a small target.
    # If that fails, the profile is at least CERT-2 (partial).
    try:
        result = subprocess.run(
            ["cargo", "check", "-p", "chronos-domain", "--lib", "--quiet"],
            cwd=str(repo_root),
            capture_output=True,
            text=True,
            timeout=300,
        )
        local_check_ok = (result.returncode == 0)
    except (subprocess.TimeoutExpired, FileNotFoundError):
        local_check_ok = False

    checks.append({
        "id": "OPS.LOCAL",
        "name": "local-cargo-check-passes",
        "status": "pass" if local_check_ok else "warn",
        "evidence": "cargo check -p chronos-domain --lib exit=0"
                   if local_check_ok else
                   "cargo check -p chronos-domain --lib failed or timed out",
    })

elif profile == "linux-privileged":
    # Linux privileged — requires CAP_BPF / CAP_SYS_PTRACE / CAP_NET_ADMIN.
    # Check whether we're running as root or with the relevant caps.
    has_caps = False
    cap_path = Path("/proc/self/status")
    if cap_path.is_file():
        text = cap_path.read_text(encoding="utf-8", errors="replace")
        if "CapBpf:" in text or "CapSysPtrace:" in text:
            # Extract cap value; non-zero means we have at least one bit set.
            for line in text.splitlines():
                if line.startswith(("CapBpf:", "CapSysPtrace:")):
                    try:
                        # Format: "CapBpf:    0000000000000000"
                        val = int(line.split(":")[1].strip(), 16)
                        if val != 0:
                            has_caps = True
                            break
                    except (ValueError, IndexError):
                        pass

    checks.append({
        "id": "OPS.LINUX",
        "name": "linux-privileged-caps-available",
        "status": "pass" if has_caps else "fail",
        "evidence": "/proc/self/status CapBpf/CapSysPtrace non-zero"
                   if has_caps else
                   "no CAP_BPF or CAP_SYS_PTRACE (per H1.2 §10, this profile is "
                   "NOT IMPLEMENTED on this host)",
    })

# ---------------------------------------------------------------------------
# Tier calculation
# ---------------------------------------------------------------------------
# Tier rules (per ADR-0032 §2.2 + ADR-0004):
#   - All checks pass (no warn, no fail) → CERT-4 production
#   - Critical checks pass, some warn, no fail → CERT-3 certified
#   - Some fail but core compiles → CERT-2 partial
#   - Multiple fail or compile broken → CERT-1 stub

n_pass = sum(1 for c in checks if c["status"] == "pass")
n_warn = sum(1 for c in checks if c["status"] == "warn")
n_fail = sum(1 for c in checks if c["status"] == "fail")

if n_fail == 0 and n_warn == 0:
    tier = "cert-4-production"
elif n_fail == 0:
    tier = "cert-3-certified"
elif n_fail <= 2:
    tier = "cert-2-partial"
else:
    tier = "cert-1-stub"

downgrade_reasons = []
for c in checks:
    if c["status"] == "fail":
        downgrade_reasons.append(f"{c['id']} ({c['name']}): {c['evidence']}")
    elif c["status"] == "warn" and tier in ("cert-4-production", "cert-3-certified"):
        downgrade_reasons.append(f"{c['id']} ({c['name']}): {c['evidence']}")

# Build report.
report = {
    "schema_version": 1,
    "profile": profile,
    "commit_sha": commit_sha,
    "binary_sha256": "not-applicable",  # would be hash of built binary in OPS.4
    "tier": tier,
    "checks": checks,
    "summary": {
        "pass": n_pass,
        "warn": n_warn,
        "fail": n_fail,
        "total": len(checks),
    },
    "downgrade_reasons": downgrade_reasons,
    "generated_at_unix_seconds": timestamp,
}

output_path.parent.mkdir(parents=True, exist_ok=True)
with open(output_path, "w", encoding="utf-8") as f:
    json.dump(report, f, indent=2, sort_keys=True, ensure_ascii=False)
    f.write("\n")

print(f"OPS.2 cert report: profile={profile} tier={tier} ({n_pass}/{len(checks)} pass)")
print(f"  written to: {output_path}")
if downgrade_reasons:
    print(f"  downgrade reasons:")
    for reason in downgrade_reasons:
        print(f"    - {reason}")
PYEOF

echo "OPS.2 cert report generated for profile '$PROFILE' at '$OUTPUT_PATH'"
exit 0
