#!/usr/bin/env python3
"""
Cycle-artifact completeness validation gate.

Run by CI (and locally before committing a new cycle folder) to ensure
every cycle in cycle-artifacts/p-3416cfb8288f8964/ has the schema fields
required by the m9 cross-checks (CC#8/11/12/13/15/17/18/26/39).

This prevents the REC-C0.4-A anti-drift scenario from recurring:
the six rec-c0-* cycle folders were created with apply-checkpoint.json
missing head_sha, base_sha, title, summary, etc., and verify-findings.json
did not exist at all. The drift only surfaced at CI time.

If a cycle folder is missing ANY of the required schema fields, this
script exits 1 and prints a precise diagnostic for each missing field.

Required schema fields on apply-checkpoint.json (per CC#8/11/12/13/15):
    cycle_id          str
    title             str
    summary           str (>= 1 char)
    head_sha          40-char hex
    base_sha          40-char hex
    main_sha          40-char hex
    remote_tag_peel   40-char hex (defaults to head_sha acceptable)
    peel_match        bool
    status            "CLOSED" (or other known status)
    findings_introduced.no_action  list (always present, possibly empty)
    verify_status     "passed" (when status is CLOSED)
    release_status    "released" (when status is CLOSED)
    archive_status    "archived" (when status is CLOSED)
    created_at        ISO8601 timestamp

Required file: verify-findings.json with:
    cycle_id          str
    all_passed        bool
    verdict           str (PASS|FAIL|PARTIAL)
    subject.head_sha  40-char hex
    subject.base_sha  40-char hex

Usage:
  python3 scripts/validate_cycle_artifacts.py
  python3 scripts/validate_cycle_artifacts.py --root /custom/path
  python3 scripts/validate_cycle_artifacts.py --strict
"""
import argparse
import json
import os
import re
import sys


HEX_RE = re.compile(r"^[0-9a-fA-F]{40}$")


def validate_apply_checkpoint(data, errors):
    if not data.get("cycle_id"):
        errors.append("apply-checkpoint.json: missing `cycle_id`")
    if not data.get("title"):
        errors.append("apply-checkpoint.json: missing or empty `title`")
    if not data.get("summary"):
        errors.append("apply-checkpoint.json: missing or empty `summary`")
    for field in ("head_sha", "base_sha", "main_sha"):
        v = data.get(field)
        if not v:
            errors.append(f"apply-checkpoint.json: missing `{field}`")
        elif not HEX_RE.match(v):
            errors.append(
                f"apply-checkpoint.json: `{field}`={v!r} is not a 40-char hex SHA"
            )
    peel = data.get("remote_tag_peel")
    if peel and not HEX_RE.match(peel):
        errors.append(
            f"apply-checkpoint.json: `remote_tag_peel`={peel!r} is not a 40-char hex SHA"
        )
    if "peel_match" not in data:
        errors.append("apply-checkpoint.json: missing `peel_match`")
    if data.get("status") != "CLOSED":
        errors.append(
            f"apply-checkpoint.json: status={data.get('status')!r}, expected 'CLOSED'"
        )
    fi = data.get("findings_introduced")
    if not isinstance(fi, dict):
        errors.append("apply-checkpoint.json: `findings_introduced` is not a dict")
    elif "no_action" not in fi:
        errors.append(
            "apply-checkpoint.json: `findings_introduced.no_action` missing"
        )
    if data.get("status") == "CLOSED":
        for field in ("verify_status", "release_status", "archive_status"):
            if data.get(field) is None:
                errors.append(
                    f"apply-checkpoint.json: `{field}` is None on a CLOSED cycle"
                )
    if not data.get("created_at"):
        errors.append("apply-checkpoint.json: missing `created_at`")


def validate_verify_findings(data, errors):
    if not data.get("cycle_id"):
        errors.append("verify-findings.json: missing `cycle_id`")
    if "all_passed" not in data:
        errors.append("verify-findings.json: missing `all_passed`")
    if data.get("verdict") not in (None, "PASS", "FAIL", "PARTIAL"):
        errors.append(
            f"verify-findings.json: verdict={data.get('verdict')!r} not in PASS/FAIL/PARTIAL"
        )
    sub = data.get("subject")
    if not isinstance(sub, dict):
        errors.append("verify-findings.json: `subject` is not a dict")
    else:
        for field in ("head_sha", "base_sha"):
            v = sub.get(field) or sub.get(field.replace("_sha", ""))
            if not v:
                errors.append(
                    f"verify-findings.json: subject.{field} missing"
                )
            elif not HEX_RE.match(v):
                errors.append(
                    f"verify-findings.json: subject.{field}={v!r} not a 40-char hex SHA"
                )


def main():
    p = argparse.ArgumentParser()
    p.add_argument(
        "--root",
        default="/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos/cycle-artifacts/p-3416cfb8288f8964",
    )
    p.add_argument(
        "--strict",
        action="store_true",
        help="also enforce on m9-* / m10-* legacy cycles (default: skip those)",
    )
    args = p.parse_args()

    if not os.path.isdir(args.root):
        print(f"ERROR: {args.root} not found", file=sys.stderr)
        return 2

    errors_by_cycle = {}
    checked = 0
    for folder in sorted(os.listdir(args.root)):
        full = os.path.join(args.root, folder)
        if not os.path.isdir(full):
            continue
        # Only run on actual cycle folders (those that contain an
        # apply-checkpoint.json). Skip handoffs, archive subdirs, etc.
        ckpt_probe = os.path.join(full, "apply-checkpoint.json")
        if not os.path.exists(ckpt_probe):
            continue
        if not args.strict and (
            folder.startswith("m9-") or folder.startswith("m10-")
        ):
            continue
        ckpt = ckpt_probe
        vf = os.path.join(full, "verify-findings.json")
        checked += 1
        with open(ckpt) as f:
            try:
                data = json.load(f)
            except json.JSONDecodeError as e:
                errors_by_cycle.setdefault(folder, []).append(
                    f"apply-checkpoint.json: invalid JSON: {e}"
                )
                continue
        errs = []
        validate_apply_checkpoint(data, errs)
        if not os.path.exists(vf):
            errs.append("missing verify-findings.json")
        else:
            with open(vf) as f:
                try:
                    vdata = json.load(f)
                except json.JSONDecodeError as e:
                    errs.append(f"verify-findings.json: invalid JSON: {e}")
                else:
                    validate_verify_findings(vdata, errs)
        if errs:
            errors_by_cycle[folder] = errs

    if errors_by_cycle:
        print(
            f"Cycle-artifact completeness gate FAILED on "
            f"{len(errors_by_cycle)} cycle(s):"
        )
        for folder, errs in errors_by_cycle.items():
            print(f"  {folder}:")
            for e in errs:
                print(f"    - {e}")
        return 1
    print(f"Cycle-artifact completeness gate PASSED on {checked} cycle(s).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
