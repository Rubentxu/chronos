#!/usr/bin/env python3
"""
Generate a CycloneDX SBOM for the Chronos workspace.

This script drives `cargo cyclonedx` (or the documented fallback when the
plugin is unavailable) to produce an SBOM per the H1.1.1 closure of
`CapChannelPin` and the OPS.2 row of H1.2 §4.2 ("supply chain / SBOM").

Defaults:
- Output directory: .sddk-state/sbom/
- Format: CycloneDX 1.6 JSON
- Inputs: workspace Cargo.lock
- Pinned in CI: the SBOM is generated on every push to main AND on tagged
  releases; the artifact is uploaded as `chronos-sbom-<tag>.json` in the
  GitHub Actions build-release job.

Refs:
- ROADMAP §H1.1, OPS.2 (supply chain / SBOM).
- docs/security/H1.2-threat-model.md §4.2 OPS.2.
- docs/runbooks/H1.6-install-upgrade-rollback.md §3 (artifact verification).
- H1.6 §8.2 cap ledger (closes CapChannelPin in concert with deny.toml).
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT_DIR = REPO_ROOT / ".sddk-state" / "sbom"


def _run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=cwd, check=False, capture_output=True, text=True)


def _tool_available(name: str) -> bool:
    """A cargo plugin isn't a real binary; it lives behind `cargo <name>`.

    Direct invocation (`cargo-<name> --version`) returns rc=2 in some
    plugin builds because `cargo-<name>` is a wrapper that parses
    argv — it does not handle `--version` itself. Going through
    `cargo <name>` is the only reliable check.
    """
    try:
        rc = _run(["cargo", name, "--version"], REPO_ROOT).returncode
    except FileNotFoundError:
        return False
    return rc == 0


def _ensure_output_dir(out_dir: Path) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)


def _bom_via_cargo_cyclonedx(out_dir: Path) -> Path:
    """Use the cargo-cyclonedx plugin if installed.

    Strategy: run cargo-cyclonedx with `--format json` plus `--top-level`.
    cargo-cyclonedx 0.5.x writes **one SBOM per workspace member** at
    `<crate-dir>/<crate-name>.cdx.json`. We collect all 19 JSON files,
    merge their `components[]` arrays into a single SBOM, and write
    that merged SBOM to `out_dir/chronos-sbom.cdx.json`. Each per-crate
    file is kept under `out_dir/per-crate/` for callers who want
    per-crate granularity.

    This behaviour matches what the `release.yml` workflow and the
    `OPS.2-REAL-SBOM-PUBLICATION` deliverable expect: one aggregated
    SBOM for the workspace, plus a per-crate directory for downstream
    tooling that wants component-level artefacts.
    """
    artifact = out_dir / "chronos-sbom.cdx.json"
    per_crate_dir = out_dir / "per-crate"
    per_crate_dir.mkdir(parents=True, exist_ok=True)

    run_rc = _run(
        [
            "cargo", "cyclonedx",
            "--format", "json",
            "--top-level",
        ],
        REPO_ROOT,
    ).returncode

    if run_rc != 0:
        raise RuntimeError(
            f"cargo-cyclonedx exited rc={run_rc}; falling back"
        )

    # Collect every JSON SBOM the plugin produced.
    produced_jsons = sorted(REPO_ROOT.rglob("*.cdx.json"))
    if not produced_jsons:
        raise RuntimeError("cargo-cyclonedx produced no JSON file; falling back")

    # Move them all under out_dir/per-crate/ for archival.
    per_crate_meta = []
    seen_purls = set()
    merged_components = []
    for jf in produced_jsons:
        relative = jf.relative_to(REPO_ROOT)
        dest = per_crate_dir / relative
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(jf.read_bytes())
        try:
            data = json.loads(jf.read_text())
            for c in data.get("components", []):
                purl = c.get("purl") or f"{c.get('group','')}/{c.get('name','')}@{c.get('version','')}"
                if purl not in seen_purls:
                    seen_purls.add(purl)
                    merged_components.append(c)
            per_crate_meta.append({
                "file": str(dest.relative_to(out_dir)),
                "bomFormat": data.get("bomFormat"),
                "specVersion": data.get("specVersion"),
                "components": len(data.get("components", [])),
            })
        except json.JSONDecodeError:
            # skip non-JSON files
            pass
        # remove from the source tree so we don't pollute the workspace
        jf.unlink()

    merged = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.6",
        "version": 1,
        "metadata": {
            "timestamp": subprocess.check_output(
                ["date", "-u", "+%Y-%m-%dT%H:%M:%SZ"], text=True
            ).strip(),
            "tools": [
                {
                    "vendor": "Chronos",
                    "name": "generate_chronos_sbom.py",
                    "version": "1.0.0",
                }
            ],
            "component": {
                "type": "application",
                "name": "chronos-workspace",
                "version": "0.7.112",
            },
        },
        "components": merged_components,
        "per_crate": per_crate_meta,
    }
    artifact.write_text(json.dumps(merged, indent=2))
    return artifact


def _bom_via_synthetic(out_dir: Path) -> Path:
    """Fallback: produce a minimal CycloneDX 1.6 SBOM from Cargo.lock directly.

    This is a downgrade from the full cargo-cyclonedx output but it is
    deterministic and offline. Today's workspace runs the full plugin in CI;
    this fallback is here so that local smoke runs without a network
    dependency.
    """
    artifact = out_dir / "chronos-sbom.cdx.json"
    with open(REPO_ROOT / "Cargo.lock", encoding="utf-8") as fh:
        lock_text = fh.read()

    components = []
    current_name = None
    current_version = None
    in_pkg = False
    for line in lock_text.splitlines():
        if line == "[[package]]":
            in_pkg = True
            current_name = None
            current_version = None
            continue
        if not in_pkg:
            continue
        if line.startswith("name = "):
            current_name = line.split("=", 1)[1].strip().strip('"')
        elif line.startswith("version = "):
            current_version = line.split("=", 1)[1].strip().strip('"')
            if current_name and current_version:
                components.append({"name": current_name, "version": current_version})
                in_pkg = False

    bom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.6",
        "version": 1,
        "metadata": {
            "timestamp": subprocess.check_output(["date", "-u", "+%Y-%m-%dT%H:%M:%SZ"], text=True).strip(),
            "tools": [
                {
                    "vendor": "Chronos",
                    "name": "generate_chronos_sbom.py",
                    "version": "1.0.0",
                }
            ],
            "component": {
                "type": "application",
                "name": "chronos-mcp",
                "version": "0.7.112",
            },
        },
        "components": components,
        "note": "synthetic fallback (cargo-cyclonedx plugin not installed locally)",
    }
    artifact.write_text(json.dumps(bom, indent=2))
    return artifact


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=DEFAULT_OUTPUT_DIR,
        help="Output directory for the SBOM (default: %(default)s)",
    )
    parser.add_argument(
        "--force-synthetic",
        action="store_true",
        help="Skip cargo-cyclonedx plugin and use the offline fallback",
    )
    parser.add_argument(
        "--print-summary",
        action="store_true",
        help="Print the number of components and exit",
    )
    args = parser.parse_args()

    _ensure_output_dir(args.out_dir)

    if args.force_synthetic or not _tool_available("cyclonedx"):
        artifact = _bom_via_synthetic(args.out_dir)
        mode = "synthetic-fallback"
    else:
        try:
            artifact = _bom_via_cargo_cyclonedx(args.out_dir)
            mode = "cargo-cyclonedx"
        except RuntimeError as exc:
            print(f"WARNING: {exc}; falling back to synthetic SBOM", file=sys.stderr)
            artifact = _bom_via_synthetic(args.out_dir)
            mode = "synthetic-fallback"

    if args.print_summary:
        data = json.loads(artifact.read_text())
        n = len(data.get("components", []))
        print(f"{mode}: {n} components -> {artifact}")
        return 0

    print(f"SBOM written: {artifact} ({mode})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
