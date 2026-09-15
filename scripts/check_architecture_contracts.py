#!/usr/bin/env python3
"""Chronos reconstruction architecture/spec fitness checks.

This checker intentionally has no third-party dependencies (Python 3.11+).
It enforces:
  * machine-ledger shape and evidence requirements for `verified` contracts;
  * a shrinking baseline of forbidden crate dependency edges;
  * no newly-added production Rust use of selected legacy APIs;
  * presence of the active convergence gates in both roadmaps.

Use --strict-no-gaps at REC-C7 to reject any remaining `gap` or `partial`
requirement whose owner is a REC-C gate.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
LEDGER = ROOT / "reconstruction-contracts.toml"

FORBIDDEN_DEPENDENCIES = {
    ("chronos-domain", "reqwest"),
    ("chronos-domain", "tokio"),
    ("chronos-domain", "tracing"),
    ("chronos-services", "chronos-store"),
    ("chronos-services", "chronos-ebpf"),
    ("chronos-services", "chronos-native"),
    ("chronos-services", "chronos-browser"),
    ("chronos-store", "chronos-native"),
}

# These tokens may remain while convergence is in progress, but CI rejects
# NEW production uses. Existing uses are removed gate-by-gate.
LEGACY_ADDITION_TOKENS = (
    "EventBus",
    "snapshot_raw(",
    "drain_fired(",
    "TraceAdapter",
    "UnsupportedOperation(",
)


def error(message: str, errors: list[str]) -> None:
    errors.append(message)
    print(f"ERROR: {message}")


def load_toml(path: Path) -> dict:
    with path.open("rb") as fh:
        return tomllib.load(fh)


def verify_ledger(ledger: dict, errors: list[str], strict_no_gaps: bool) -> None:
    policy = ledger.get("policy", {})
    allowed = set(policy.get("allowed_statuses", []))
    requirements = ledger.get("requirement", [])
    ids: set[str] = set()

    if not requirements:
        error("reconstruction contract ledger has no requirements", errors)
        return

    for req in requirements:
        req_id = req.get("id")
        if not req_id or req_id in ids:
            error(f"missing or duplicate requirement id: {req_id!r}", errors)
            continue
        ids.add(req_id)

        status = req.get("status")
        if status not in allowed:
            error(f"{req_id}: invalid status {status!r}", errors)

        if not req.get("owner_gate"):
            error(f"{req_id}: owner_gate is mandatory", errors)

        if status == "verified":
            for field in ("evidence", "uat", "verify"):
                value = req.get(field)
                if not value:
                    error(f"{req_id}: verified contract missing {field}", errors)

        if strict_no_gaps and str(req.get("owner_gate", "")).startswith("REC-C"):
            if status in {"gap", "partial", "blocked"}:
                error(f"{req_id}: REC-C7 strict close rejects status={status}", errors)


def cargo_dependency_edges() -> set[tuple[str, str]]:
    edges: set[tuple[str, str]] = set()
    for cargo in (ROOT / "crates").glob("*/Cargo.toml"):
        data = load_toml(cargo)
        package = data.get("package", {}).get("name")
        if not package:
            continue
        for section in ("dependencies", "build-dependencies"):
            for dependency in data.get(section, {}).keys():
                edges.add((package, dependency))
    return edges


def verify_dependency_baseline(ledger: dict, errors: list[str]) -> None:
    current = cargo_dependency_edges() & FORBIDDEN_DEPENDENCIES
    configured = set()
    for item in ledger.get("architecture", {}).get("known_dependency_violations", []):
        if "->" not in item:
            error(f"invalid dependency baseline entry: {item}", errors)
            continue
        source, target = item.split("->", 1)
        configured.add((source, target))

    unexpected = sorted(current - configured)
    stale = sorted(configured - current)
    for source, target in unexpected:
        error(f"new forbidden dependency edge: {source} -> {target}", errors)
    for source, target in stale:
        error(
            f"stale architecture debt baseline: {source} -> {target} no longer exists; remove the waiver",
            errors,
        )


def git_diff(base: str) -> str:
    result = subprocess.run(
        ["git", "diff", "--unified=0", f"{base}..HEAD", "--", "crates"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "git diff failed")
    return result.stdout


def verify_no_new_legacy(errors: list[str]) -> None:
    base = os.environ.get("CHRONOS_CONTRACT_BASE_REF")
    if not base:
        print("WARN: CHRONOS_CONTRACT_BASE_REF unset; skipping added-line legacy scan")
        return

    try:
        diff = git_diff(base)
    except RuntimeError as exc:
        error(f"cannot evaluate legacy additions against {base}: {exc}", errors)
        return

    current_file: str | None = None
    for line in diff.splitlines():
        if line.startswith("+++ b/"):
            current_file = line[6:]
            continue
        if not current_file or not current_file.endswith(".rs"):
            continue
        if "/tests/" in current_file or current_file.endswith("/tests.rs"):
            continue
        if not line.startswith("+") or line.startswith("+++"):
            continue
        added = line[1:]
        for token in LEGACY_ADDITION_TOKENS:
            if token in added:
                error(f"new legacy token {token!r} in {current_file}: {added.strip()}", errors)


def verify_roadmap_markers(errors: list[str]) -> None:
    paths = [
        ROOT / "docs" / "ROADMAP.md",
        ROOT
        / "docs"
        / "chronos-agentic-reconstruction"
        / "docs"
        / "roadmap"
        / "ROADMAP.md",
    ]
    for path in paths:
        if not path.exists():
            error(f"missing roadmap: {path.relative_to(ROOT)}", errors)
            continue
        text = path.read_text(encoding="utf-8")
        for marker in ("REC-C0", "REC-C7"):
            if marker not in text:
                error(f"{path.relative_to(ROOT)} missing convergence marker {marker}", errors)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--strict-no-gaps",
        action="store_true",
        help="REC-C7 close mode: fail remaining partial/gap/blocked REC-C contracts",
    )
    args = parser.parse_args()

    errors: list[str] = []
    if not LEDGER.exists():
        error("missing reconstruction-contracts.toml", errors)
        return 1

    ledger = load_toml(LEDGER)
    verify_ledger(ledger, errors, args.strict_no_gaps)
    verify_dependency_baseline(ledger, errors)
    verify_no_new_legacy(errors)
    verify_roadmap_markers(errors)

    if errors:
        print(f"\nArchitecture/spec fitness gate FAILED with {len(errors)} finding(s).")
        return 1
    print("Architecture/spec fitness gate PASSED.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
