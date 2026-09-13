#!/usr/bin/env python3
"""Regenerate the SHA-256 column of archive-manifest Artifact indexes (CC#4).

Why this exists
---------------
Every cycle ends by rewriting the `## Artifact index` table of its
`archive-manifest.md`: the rows are hashes of *other* files in the same commit,
so they can only be computed after the file contents are frozen. Doing that by
hand (or with a throwaway script in `$TMPDIR`) is the ritual that
`FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO` complains about. This script is the
in-repo, reviewable version of that ritual, and `--check` is the machine
verification that the ritual was actually performed.

Rules (mirroring CC#4 in the vault drift sweep)
-----------------------------------------------
1. A row whose path resolves to the manifest itself is skipped: a file cannot
   contain its own hash, and CC#4 already excludes that row. Such rows are
   reported as `self`.
2. A row whose path does not exist is left byte-for-byte untouched and reported
   as `missing`, so the drift gate keeps reporting the mistake instead of the
   script quietly blanking it.
3. Rewriting one manifest can change the hash of a manifest that *another*
   manifest lists, so with `--all` the script iterates to a fixpoint (bounded).

Usage
-----
    scripts/regen_manifest_index_shas.py                 # rewrite all archives
    scripts/regen_manifest_index_shas.py --dry-run       # report, write nothing
    scripts/regen_manifest_index_shas.py --check         # exit 1 if any row is stale
    scripts/regen_manifest_index_shas.py <manifest>...   # rewrite only these

Exit status: 0 on success (including "nothing to do"), 1 when `--check` finds a
stale row, 2 on usage/IO error. Run from the repository root (or any cwd: the
default manifest set is resolved relative to this script's parent directory).
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import sys

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ARCHIVE_GLOB_ROOT = ".sddk-knowledge"
MAX_PASSES = 5

# | Kind | `path` | `sha` |  -- the exact row shape CC#4 parses.
ROW = re.compile(r"^\|([^|]+)\|\s*`([^`]+)`\s*\|\s*`([0-9a-fA-F]{64})`\s*\|\s*$")
ZERO_SHA = "0" * 64


def sha256(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def default_manifests(root: str) -> list[str]:
    """All archive manifests under `<root>/.sddk-knowledge/*/changes/archive/`."""
    out: list[str] = []
    base = os.path.join(root, ARCHIVE_GLOB_ROOT)
    if not os.path.isdir(base):
        return out
    for project in sorted(os.listdir(base)):
        archive = os.path.join(base, project, "changes", "archive")
        if not os.path.isdir(archive):
            continue
        for change in sorted(os.listdir(archive)):
            candidate = os.path.join(archive, change, "archive-manifest.md")
            if os.path.isfile(candidate):
                out.append(candidate)
    return out


def rows_of(manifest: str) -> list[tuple[str, str, str]]:
    """Return (label, path, sha) triples for rows matching the CC#4 shape."""
    found = []
    with open(manifest, encoding="utf-8") as fh:
        for line in fh.read().split("\n"):
            m = ROW.match(line)
            if m:
                found.append((m.group(1), m.group(2), m.group(3)))
    return found


def rewrite(manifest: str, root: str, *, write: bool) -> dict:
    """Recompute every row of one manifest.

    Returns a report dict: updated [(path, old, new)], self [path], missing [path].
    """
    manifest_abs = os.path.normpath(os.path.abspath(manifest))
    report = {"updated": [], "self": [], "missing": []}
    out: list[str] = []
    with open(manifest, encoding="utf-8") as fh:
        lines = fh.read().split("\n")
    for line in lines:
        m = ROW.match(line)
        if not m:
            out.append(line)
            continue
        _, path, current = m.groups()
        target_abs = os.path.normpath(os.path.join(root, path))
        if target_abs == manifest_abs:
            # Rule 1: a file cannot contain its own hash. Keep the committed
            # placeholder (normally 64 zeros) so the bytes stay stable.
            report["self"].append(path)
            out.append(line)
            continue
        if not os.path.exists(target_abs):
            # Rule 2: never blank a dangling row; let the drift gate report it.
            report["missing"].append(path)
            out.append(line)
            continue
        new = sha256(target_abs)
        if new != current:
            label = m.group(1)
            out.append(f"|{label}| `{path}` | `{new}` |")
            report["updated"].append((path, current, new))
            continue
        out.append(line)
    if write and report["updated"]:
        with open(manifest, "w", encoding="utf-8") as fh:
            fh.write("\n".join(out))
    return report


def rel(path: str, root: str) -> str:
    try:
        return os.path.relpath(path, root)
    except ValueError:  # pragma: no cover - different drive on Windows
        return path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Regenerate archive-manifest Artifact index SHA-256 rows (CC#4).",
    )
    parser.add_argument(
        "manifests",
        nargs="*",
        help="archive-manifest.md paths; default: every manifest under "
        f"{ARCHIVE_GLOB_ROOT}/*/changes/archive/",
    )
    parser.add_argument(
        "--root",
        default=REPO_ROOT,
        help="repository root used to resolve row paths (default: script's parent)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="write nothing; exit 1 if any row is stale (this is the gate)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="write nothing; exit 0 even when rows are stale",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="also print a line for every manifest that needed no change",
    )
    args = parser.parse_args(argv)

    root = os.path.abspath(args.root)
    manifests = args.manifests or default_manifests(root)
    if not manifests:
        print("regen-manifest-index-shas: no archive manifests found", file=sys.stderr)
        return 2

    if args.check and args.dry_run:
        print("regen-manifest-index-shas: --check and --dry-run are exclusive", file=sys.stderr)
        return 2

    write = not (args.check or args.dry_run)
    total_updated = 0
    stale = False
    missing_any: list[tuple[str, str]] = []

    for _ in range(MAX_PASSES):
        pass_updated = 0
        for manifest in manifests:
            if not os.path.isfile(manifest):
                print(f"regen-manifest-index-shas: missing manifest {manifest}", file=sys.stderr)
                return 2
            report = rewrite(manifest, root, write=write)
            pass_updated += len(report["updated"])
            total_updated += len(report["updated"]) if write else 0
            if report["updated"]:
                stale = True
            # A clean manifest (even one carrying a preserved self row) says
            # nothing useful, so stay silent unless --verbose or something
            # actually needs attention. 75 lines of "0 row(s) stale" is noise
            # in a CI log that is scanned for offenders.
            if report["updated"] or report["missing"] or args.verbose:
                state = "updated" if write else "stale"
                detail = f"{len(report['updated'])} row(s) {state}"
                if report["self"]:
                    detail += f", self={len(report['self'])}"
                if report["missing"]:
                    detail += f", missing={len(report['missing'])}"
                print(f"{rel(manifest, root)}: {detail}")
            # Always name the offending rows: `--check` exists to be actionable
            # in CI logs, so suppressing the row under a flag would defeat it.
            for path, old, new in report["updated"]:
                arrow = f"{old} -> {new}" if write else f"stale {old} (expected {new})"
                print(f"  {rel(manifest, root)} :: {path}: {arrow}")
            for path in report["missing"]:
                missing_any.append((rel(manifest, root), path))
        if not write or pass_updated == 0:
            break
    else:
        print(
            f"regen-manifest-index-shas: still changing after {MAX_PASSES} passes; "
            "manifests reference each other cyclically",
            file=sys.stderr,
        )
        return 2

    for manifest, path in missing_any:
        print(
            f"regen-manifest-index-shas: warning: {manifest} lists `{path}` which does not exist",
            file=sys.stderr,
        )

    if args.check:
        if stale:
            print("regen-manifest-index-shas: DRIFT: stale SHA row(s) in the table above")
            return 1
        print(f"regen-manifest-index-shas: clean ({len(manifests)} manifest(s) checked)")
        return 0

    if args.dry_run:
        if stale:
            print("regen-manifest-index-shas: dry-run: stale row(s) listed above (rerun without --dry-run to rewrite)")
        else:
            print(f"regen-manifest-index-shas: dry-run: nothing to do ({len(manifests)} manifest(s) already correct)")
        return 0

    if write:
        if total_updated:
            print(
                f"regen-manifest-index-shas: {total_updated} row(s) rewritten "
                f"across {len(manifests)} manifest(s)"
            )
        else:
            print(f"regen-manifest-index-shas: nothing to do ({len(manifests)} manifest(s) already correct)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
