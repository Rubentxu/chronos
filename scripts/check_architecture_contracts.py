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


def _cfg_test_lines(path: Path) -> set[int]:
    """Return 1-indexed line numbers inside any `#[cfg(test)]` block.

    The legacy-token scan must skip test code. The path-based exclusion
    (`/tests/`) catches files under `tests/` directories, but Rust also
    allows `#[cfg(test)] mod tests { ... }` inside any source file. We
    scan the file at HEAD and track brace depth from each opening
    `#[cfg(test)]` so we know which lines are inside the test scope.
    """
    if not path.exists() or not path.is_file():
        return set()
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return set()
    in_cfg_test = False
    depth = 0
    test_lines: set[int] = set()
    for idx, raw in enumerate(text.splitlines(), start=1):
        line = raw.strip()
        if not in_cfg_test:
            if line == "#[cfg(test)]" or line.startswith("#[cfg(test)] "):
                in_cfg_test = True
                depth = 0
        if in_cfg_test:
            test_lines.add(idx)
            for ch in raw:
                if ch == "{":
                    depth += 1
                elif ch == "}":
                    depth -= 1
                    if depth == 0:
                        in_cfg_test = False
                        break
    return test_lines


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

    # Cache: for each file, the set of 1-indexed line numbers inside any
    # `#[cfg(test)]` block at HEAD. The diff is --unified=0 so added line
    # numbers map 1:1 to file line numbers at HEAD.
    cfg_test_cache: dict[str, set[int]] = {}

    def is_test_line(file_path: str, line_no: int) -> bool:
        if file_path not in cfg_test_cache:
            cfg_test_cache[file_path] = _cfg_test_lines(ROOT / file_path)
        return line_no in cfg_test_cache[file_path]

    current_file: str | None = None
    current_added_line_no = 0
    for line in diff.splitlines():
        if line.startswith("+++ b/"):
            current_file = line[6:]
            current_added_line_no = 0
            continue
        if not current_file or not current_file.endswith(".rs"):
            continue
        if "/tests/" in current_file or current_file.endswith("/tests.rs"):
            continue
        if line.startswith("+++") or line.startswith("---"):
            continue
        # Diff --unified=0: hunk headers `@@ -a,b +c,d @@` give the new-side
        # start line `c`. Added lines (`+`) follow at consecutive new line
        # numbers until the next hunk or non-`+` line.
        if line.startswith("@@"):
            import re as _re

            m = _re.search(r"\+(\d+)", line)
            if m:
                current_added_line_no = int(m.group(1)) - 1
            continue
        if line.startswith("+"):
            current_added_line_no += 1
            added = line[1:]
            if is_test_line(current_file, current_added_line_no):
                continue
            for token in LEGACY_ADDITION_TOKENS:
                if token in added:
                    error(
                        f"new legacy token {token!r} in {current_file}:{current_added_line_no}: {added.strip()}",
                        errors,
                    )
        elif not line.startswith("-"):
            # Context lines (no prefix or space prefix) also advance the
            # new-side line counter so the next `+` line gets the right
            # line number.
            current_added_line_no += 1


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


def verify_legacy_evb_inventory(errors: list[str], strict: bool) -> None:
    """Run the REC-C2 legacy EventBus/queue ratchet as part of this gate."""
    script = ROOT / "scripts" / "check_legacy_evb.py"
    if not script.exists():
        error("missing scripts/check_legacy_evb.py", errors)
        return
    cmd = [sys.executable, str(script)]
    if strict:
        cmd.append("--strict")
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        tail = (result.stdout or result.stderr or "").strip()
        error(f"legacy-evb ratchet failed:\n{tail}", errors)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--strict-no-gaps",
        action="store_true",
        help="REC-C7 close mode: fail remaining partial/gap/blocked REC-C contracts",
    )
    parser.add_argument(
        "--strict-legacy",
        action="store_true",
        help="REC-C2 close mode: fail on any CANONICAL destructive EventBus read",
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
    verify_legacy_evb_inventory(errors, args.strict_legacy)
    verify_roadmap_markers(errors)

    if errors:
        print(f"\nArchitecture/spec fitness gate FAILED with {len(errors)} finding(s).")
        return 1
    print("Architecture/spec fitness gate PASSED.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
