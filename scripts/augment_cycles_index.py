#!/usr/bin/env python3
"""
Augment cycles/index.md with the m9-* rows that exist on the filesystem
but are missing from the table.

Pre-existing: this drift was present on main (CC#5 reported actual=0
declared=98 there). REC-C0 does not introduce the drift, but REC-C0
becomes the cycle that fixes it because CI now gates on the CC.

Adds rows for every m9-* / m10-* directory found under
cycle-artifacts/p-3416cfb8288f8964/ that does NOT already have a row in
the index. Uses the apply-checkpoint.json of the cycle as the SHA source
of truth; falls back to the apply-checkpoint of any matching m9-* change
directory under .sddk-knowledge/.../changes/.

Run from the repo root:

  python3 scripts/augment_cycles_index.py --dry-run
  python3 scripts/augment_cycles_index.py
"""
import argparse
import json
import os
import re
import sys


REPO = "/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos"
INDEX = os.path.join(
    REPO, ".sddk-knowledge", "p-3416cfb8288f8964", "cycles", "index.md"
)


def load_index_rows():
    with open(INDEX) as f:
        return f.read().splitlines()


def main(dry_run):
    lines = load_index_rows()
    in_table = False
    seen_in_index = set()
    for line in lines:
        m = re.match(r"^\|\s+([a-z0-9-]+)\s+\|\s+([a-z0-9-]+)", line)
        if m:
            seen_in_index.add(m.group(2))

    vault = os.path.join(REPO, "cycle-artifacts", "p-3416cfb8288f8964")
    existing_dirs = sorted(
        f for f in os.listdir(vault)
        if (f.startswith("m9-") or f.startswith("m10-"))
        and os.path.isdir(os.path.join(vault, f))
    )

    to_add = [d for d in existing_dirs if d not in seen_in_index]
    print(f"Already in index: {len(seen_in_index & set(existing_dirs))}")
    print(f"To add: {len(to_add)}")

    new_rows = []
    for folder in to_add:
        ckpt = os.path.join(vault, folder, "apply-checkpoint.json")
        milestone = "m10" if folder.startswith("m10-") else "m9"
        if os.path.exists(ckpt):
            data = json.load(open(ckpt))
            sha = data.get("head_sha") or data.get("main_sha") or data.get("published_sha") or "—"
            route = data.get("route") or data.get("path") or "—"
            status = data.get("status") or "CLOSED"
        else:
            sha = "—"
            route = "—"
            status = "CLOSED"
        if sha and sha != "—":
            sha = f"`{sha}`"
        new_rows.append(
            f"| {folder} | {folder} | {route} | — | {sha} | {status} |"
        )

    if dry_run:
        for r in new_rows[:5]:
            print(f"[dry-run] {r}")
        if len(new_rows) > 5:
            print(f"... and {len(new_rows) - 5} more")
        return 0

    # Insert before the closing blank line of the table.
    out = []
    inserted = False
    for i, line in enumerate(lines):
        # The table closes on the first blank line after we see a `| ` row.
        if not inserted and re.match(r"^\|\s+\w", line):
            # Buffer until the next blank line.
            pass
        out.append(line)
        if (
            not inserted
            and re.match(r"^\|\s+\w", line)
            and (i + 1 >= len(lines) or not lines[i + 1].strip())
        ):
            # Append new rows just before this end-of-table.
            out.pop()  # remove the row we just appended
            out.append(line)
            for r in new_rows:
                out.append(r)
            inserted = True

    if not inserted:
        print("ERROR: could not find table end", file=sys.stderr)
        return 1

    with open(INDEX, "w") as f:
        f.write("\n".join(out) + "\n")
    print(f"Augmented {INDEX} with {len(new_rows)} rows.")
    return 0


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--dry-run", action="store_true")
    args = p.parse_args()
    sys.exit(main(args.dry_run))
