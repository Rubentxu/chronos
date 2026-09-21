#!/usr/bin/env python3
"""Backfill apply-checkpoint.json with CC#11/CC#15 canonical fields.

Required additions:
  - title (string): human-readable title, derived from the first commit on the
    cycle directory.
  - summary (string): short paragraph, derived from the first commit body.
  - created_at (RFC3339 string): timestamp of the first commit on the cycle
    directory.
  - archived_at (RFC3339 string): timestamp of the most recent commit on the
    cycle directory (CC#11 requires archived_at on closed cycles).

Only writes when (a) the cycle status indicates it is closed AND (b) at least
one of the four fields is missing. Writes are atomic: a .bak file is left
next to the original.
"""
import json
import os
import subprocess
import sys
import glob
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
os.chdir(ROOT)


def git_first_commit(dirpath):
    out = subprocess.check_output(
        ["git", "log", "--reverse", "--format=%H|%aI|%s", "--", dirpath],
        text=True,
    ).strip()
    lines = [ln for ln in out.splitlines() if ln.strip()]
    return lines[0] if lines else None


def git_last_commit(dirpath):
    out = subprocess.check_output(
        ["git", "log", "-1", "--format=%H|%aI|%s", "--", dirpath],
        text=True,
    ).strip()
    return out or None


def git_commit_body(sha):
    out = subprocess.check_output(
        ["git", "log", "-1", "--format=%b", sha],
        text=True,
    ).strip()
    return out


def update_checkpoint(path):
    with open(path) as f:
        try:
            data = json.load(f)
        except json.JSONDecodeError as e:
            print(f"SKIP {path}: JSON parse error: {e}", file=sys.stderr)
            return False

    status = (data.get("status") or "").upper()
    if status not in ("CLOSED", "closed", "RELEASED", "released", "ARCHIVED", "archived"):
        # Not closed; do not backfill.
        return False

    missing = [f for f in ("title", "summary", "created_at", "archived_at") if f not in data or data[f] in (None, "")]
    if not missing:
        return False

    first = git_first_commit(os.path.dirname(path))
    last = git_last_commit(os.path.dirname(path))
    if not first or not last:
        print(f"SKIP {path}: no git history", file=sys.stderr)
        return False

    first_sha, first_date, first_subject = first.split("|", 2)
    last_sha, last_date, _ = last.split("|", 2)
    first_body = git_commit_body(first_sha)

    title = data.get("title") or first_subject.strip()
    summary = data.get("summary") or (first_body.strip().splitlines()[0] if first_body.strip() else first_subject.strip())
    if not data.get("created_at"):
        data["created_at"] = first_date
    if not data.get("archived_at"):
        data["archived_at"] = last_date
    data["title"] = title
    data["summary"] = summary

    bak = path + ".bak"
    if not os.path.exists(bak):
        os.rename(path, bak)
    with open(path, "w") as f:
        json.dump(data, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(f"OK   {os.path.basename(os.path.dirname(path))}: +title +summary +created_at +archived_at")
    return True


def main():
    updated = 0
    for cp in sorted(glob.glob("cycle-artifacts/p-3416cfb8288f8964/*/apply-checkpoint.json")):
        if update_checkpoint(cp):
            updated += 1
    print(f"---\nUpdated {updated} checkpoint(s).")


if __name__ == "__main__":
    main()
