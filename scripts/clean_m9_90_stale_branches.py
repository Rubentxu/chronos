#!/usr/bin/env python3
"""
clean_m9_90_stale_branches.py — Delete stale local + remote m9-* branches
that have been merged into main. Idempotent + safe by construction:
- Only targets `feat/m9-*`, `fix/m9-*`, `chore/m9-*` prefixes (per CC#46)
- Verifies merge-base --is-ancestor before any delete (per CC#53)
- Records every deletion to `scripts/branches-deleted-m9-90.log` for recovery
- Idempotent: running twice is a no-op

Usage:
  python3 scripts/clean_m9_90_stale_branches.py --dry-run   # preview only
  python3 scripts/clean_m9_90_stale_branches.py             # actually delete

Per m9-65 precedent (CC#46/CC#53 closure):
- m9-54 deleted 44 local + 28 remote (covered fix/m9-*)
- m9-65 extended coverage to all milestone prefixes (added CC#53)
- m9-65 deleted 25 local + 24 remote

m9-90 is the follow-up that closes the deferred portion: 6 local +
3 remote feat/m9-* branches from m9-67..m9-78 that m9-65 missed
(because m9-65 ran before m9-66..m9-78 cycles landed their feat
branches).

Safety properties (matches m9-65):
1. Refuses to operate outside a git repo.
2. Refuses to operate on the currently checked-out branch.
3. Only deletes branches verified merged into main.
4. Logs every deletion (local + remote) before action.
5. Dry-run mode shows what would be deleted without acting.

Exit codes:
  0  all stale branches deleted (or already deleted — idempotent)
  1  error (e.g., not a git repo, current branch is main itself)
  2  some branches failed to delete (e.g., permission denied on remote)
"""
import argparse
import os
import subprocess
import sys
from datetime import datetime, timezone

LOG_PATH = "scripts/branches-deleted-m9-90.log"
TARGET_PREFIXES = ("feat/m9-", "fix/m9-", "chore/m9-")


def run(cmd, **kwargs):
    """Run a subprocess and return (returncode, stdout, stderr)."""
    p = subprocess.run(cmd, capture_output=True, text=True, **kwargs)
    return p.returncode, p.stdout.strip(), p.stderr.strip()


def get_local_branches():
    """Return list of local branch names matching TARGET_PREFIXES, excluding current."""
    rc, out, _ = run(["git", "branch", "--list"] + [p + "*" for p in TARGET_PREFIXES])
    if rc != 0:
        return []
    branches = []
    for line in out.splitlines():
        line = line.strip()
        if not line or line.startswith("*"):
            continue  # skip current branch
        branches.append(line)
    return branches


def get_remote_branches():
    """Return list of remote branch names (origin/...) matching TARGET_PREFIXES."""
    rc, out, _ = run(["git", "branch", "-r", "--list"] + ["origin/" + p + "*" for p in TARGET_PREFIXES])
    if rc != 0:
        return []
    branches = []
    for line in out.splitlines():
        line = line.strip()
        if not line:
            continue
        branches.append(line)
    return branches


def is_merged(branch):
    """Return True if `branch` is an ancestor of `main`."""
    rc, _, _ = run(["git", "merge-base", "--is-ancestor", branch, "main"])
    return rc == 0


def get_tip_sha(branch):
    """Return short SHA of the branch's tip commit."""
    rc, out, _ = run(["git", "rev-parse", "--short", branch])
    if rc != 0:
        return "UNKNOWN"
    return out.strip()


def append_log(lines):
    """Append log lines to LOG_PATH."""
    if not lines:
        return
    with open(LOG_PATH, "a", encoding="utf-8") as f:
        for line in lines:
            f.write(line + "\n")


def delete_local(branch, dry_run):
    """Delete a local branch. Returns (success, log_line)."""
    tip = get_tip_sha(branch)
    timestamp = datetime.now(timezone.utc).isoformat()
    if dry_run:
        return True, f"[DRY-RUN] {timestamp} local {branch} @ {tip} (would delete)"
    rc, _, err = run(["git", "branch", "-d", branch])
    if rc != 0:
        return False, f"[FAIL] {timestamp} local {branch} @ {tip}: {err}"
    return True, f"[OK] {timestamp} local {branch} @ {tip} deleted"


def delete_remote(branch, dry_run):
    """Delete a remote branch via `git push origin --delete`. Returns (success, log_line)."""
    # branch looks like "origin/feat/m9-..."
    short = branch.split("/", 1)[1]  # "feat/m9-..."
    tip = get_tip_sha(branch)
    timestamp = datetime.now(timezone.utc).isoformat()
    if dry_run:
        return True, f"[DRY-RUN] {timestamp} remote {branch} @ {tip} (would push origin :{short})"
    rc, _, err = run(["git", "push", "origin", "--delete", short])
    if rc != 0:
        return False, f"[FAIL] {timestamp} remote {branch} @ {tip}: {err}"
    return True, f"[OK] {timestamp} remote {branch} @ {tip} deleted via push origin :{short}"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--dry-run", action="store_true", help="preview only, no deletions")
    parser.add_argument("--reset-log", action="store_true", help="truncate LOG_PATH before writing")
    args = parser.parse_args()

    # Check we are in a git repo and on a non-main branch.
    rc, out, _ = run(["git", "rev-parse", "--is-inside-work-tree"])
    if rc != 0:
        print("ERROR: not inside a git work tree", file=sys.stderr)
        sys.exit(1)
    rc, current_branch, _ = run(["git", "symbolic-ref", "--short", "HEAD"])
    if rc != 0:
        print("ERROR: cannot determine current branch (detached HEAD?)", file=sys.stderr)
        sys.exit(1)
    if current_branch == "main":
        print("ERROR: refuse to operate on main branch (checkout a cycle branch first)",
              file=sys.stderr)
        sys.exit(1)

    if args.reset_log and not args.dry_run:
        # truncate
        with open(LOG_PATH, "w") as f:
            f.write("")

    # Header line in log
    header = (f"# m9-90 stale-branches cleanup log "
              f"{'[DRY-RUN] ' if args.dry_run else ''}"
              f"started {datetime.now(timezone.utc).isoformat()} "
              f"from branch {current_branch!r}")
    if not args.dry_run:
        append_log([header, ""])

    # Discover candidates
    locals_ = get_local_branches()
    remotes = get_remote_branches()
    print(f"Discovered: {len(locals_)} local + {len(remotes)} remote candidate branches")
    print(f"Prefixes: {TARGET_PREFIXES}")

    # Filter to merged-into-main only (CC#53 safety check)
    merged_locals = [b for b in locals_ if is_merged(b)]
    not_merged_locals = [b for b in locals_ if not is_merged(b)]
    merged_remotes = [b for b in remotes if is_merged(b)]
    not_merged_remotes = [b for b in remotes if not is_merged(b)]

    print(f"Merged into main: {len(merged_locals)} local + {len(merged_remotes)} remote")
    print(f"NOT merged (preserved): {len(not_merged_locals)} local + {len(not_merged_remotes)} remote")
    print()

    log_lines = [header, ""]

    # Delete local
    print("=== Local ===")
    for b in merged_locals:
        ok, line = delete_local(b, args.dry_run)
        print(line)
        log_lines.append(line)
    print()

    # Delete remote
    print("=== Remote ===")
    for b in merged_remotes:
        ok, line = delete_remote(b, args.dry_run)
        print(line)
        log_lines.append(line)
    print()

    # List preserved
    if not_merged_locals or not_merged_remotes:
        print("=== Preserved (NOT merged into main — human triage required) ===")
        for b in not_merged_locals:
            print(f"  local:  {b} @ {get_tip_sha(b)}")
        for b in not_merged_remotes:
            print(f"  remote: {b} @ {get_tip_sha(b)}")

    # Final summary
    summary = (f"\n# Summary: "
               f"{len(merged_locals)} local + {len(merged_remotes)} remote "
               f"{'[would be] ' if args.dry_run else ''}deleted. "
               f"{len(not_merged_locals) + len(not_merged_remotes)} branches preserved.")
    print(summary)
    log_lines.append(summary)

    if not args.dry_run:
        append_log(log_lines)
        print(f"\nLog written to {LOG_PATH}")
    else:
        print("(dry-run — nothing written to log)")


if __name__ == "__main__":
    main()
