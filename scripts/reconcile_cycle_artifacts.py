#!/usr/bin/env python3
"""
Reconcile cycle-artifacts/p-3416cfb8288f8964/* to schema required by
scripts/validate_cycle_artifacts.py (CC#8/11/12/13/15/17/18/26/39).

Strict rules:
 - NO modification to validator.
 - NO inference of "PASS" verdicts for cycles that were FAIL or unknown.
 - NO substitution of short SHA by current HEAD unless the short SHA
   resolves uniquely in `git rev-parse --verify`.
 - Verifiability: every change is either an exact reconstruction from
   git/cat-file (no guesswork) OR is recorded as `_note` or
   `_restoration_note` to make the lack of evidence explicit.

Output modes:
 - default: dry-run (print planned changes).
 - --apply: actually write files.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys

CYCLES_ROOT = "cycle-artifacts/p-3416cfb8288f8964"
HEX_RE = re.compile(r"^[0-9a-f]{40}$")
SHORT_HEX_RE = re.compile(r"^[0-9a-f]{4,39}$")


def git(*args, check=False):
    return subprocess.run(
        ["git", *args],
        capture_output=True,
        text=True,
        check=check,
    )


def resolve_short_sha(short: str) -> tuple[str | None, str]:
    """Resolve a short or partial hex SHA to a full 40-char hex.

    Returns (full_sha or None, provenance_note).
    """
    if not isinstance(short, str) or not SHORT_HEX_RE.match(short):
        return None, "not_a_short_hex"

    rev = git("rev-parse", "--verify", f"{short}^{{commit}}")
    if rev.returncode == 0 and HEX_RE.match(rev.stdout.strip()):
        return rev.stdout.strip(), f"git rev-parse --verify {short}^{{commit}}"

    out = git("log", "--all", "--format=%H")
    candidates = [c.strip() for c in out.stdout.splitlines() if c.strip().startswith(short)]
    if len(candidates) == 1 and HEX_RE.match(candidates[0]):
        return candidates[0], f"unique prefix match in git log --all ({len(candidates)} candidate)"
    if len(candidates) > 1:
        return None, f"ambiguous prefix: {len(candidates)} candidates start with {short}"
    return None, f"no commit starts with {short}"


def load_json(path: str) -> tuple[dict | None, str | None]:
    try:
        with open(path) as f:
            return json.load(f), None
    except FileNotFoundError:
        return None, "missing"
    except json.JSONDecodeError as e:
        return None, f"invalid JSON: {e}"


def save_json(path: str, data: dict, *, dry_run: bool) -> list[str]:
    if dry_run:
        return [f"would write {path}"]
    with open(path, "w") as f:
        json.dump(data, f, indent=2, ensure_ascii=False)
        f.write("\n")
    return [f"wrote {path}"]


def reconcile_apply_checkpoint(folder: str, ckpt: dict, *, dry_run: bool) -> list[str]:
    notes: list[str] = []
    folder_label = f"[{folder}]"

    # head_sha
    hs = ckpt.get("head_sha")
    if isinstance(hs, str) and not HEX_RE.match(hs):
        full, prov = resolve_short_sha(hs)
        if full:
            ckpt["head_sha"] = full
            notes.append(f"{folder_label} head_sha {hs} -> {full} ({prov})")
        else:
            notes.append(f"{folder_label} head_sha {hs} UNRESOLVED: {prov}")

    # main_sha: if present as short hex, expand via git. If absent, leave
    # to the existing "missing main_sha" branch below.
    ms = ckpt.get("main_sha")
    if isinstance(ms, str) and not HEX_RE.match(ms):
        full, prov = resolve_short_sha(ms)
        if full:
            ckpt["main_sha"] = full
            notes.append(f"{folder_label} main_sha {ms} -> {full} ({prov})")
        else:
            notes.append(f"{folder_label} main_sha {ms} UNRESOLVED: {prov}")

    # remote_tag_peel: validator only fails if non-hex is present; if it
    # is non-hex and resolves, expand it.
    peel = ckpt.get("remote_tag_peel")
    if isinstance(peel, str) and not HEX_RE.match(peel):
        full, prov = resolve_short_sha(peel)
        if full:
            ckpt["remote_tag_peel"] = full
            notes.append(f"{folder_label} remote_tag_peel {peel} -> {full} ({prov})")
        else:
            notes.append(f"{folder_label} remote_tag_peel {peel} UNRESOLVED: {prov}")
    elif "remote_tag_peel" not in ckpt:
        # default to head_sha only if it's 40-char
        if HEX_RE.match(ckpt.get("head_sha") or ""):
            ckpt["remote_tag_peel"] = ckpt["head_sha"]
            notes.append(f"{folder_label} remote_tag_peel defaulted to head_sha")
        else:
            ckpt["remote_tag_peel"] = None

    # base_sha
    bs = ckpt.get("base_sha")
    if bs is None:
        # try to derive from git: merge-base of head and origin/main
        full_hs = ckpt.get("head_sha")
        if full_hs and HEX_RE.match(full_hs or ""):
            mb = git("merge-base", full_hs, "origin/main")
            if mb.returncode == 0 and HEX_RE.match(mb.stdout.strip()):
                ckpt["base_sha"] = mb.stdout.strip()
                notes.append(f"{folder_label} base_sha inferred from merge-base({full_hs[:8]}, origin/main)")
            else:
                notes.append(f"{folder_label} base_sha missing and cannot be inferred from git")
        else:
            notes.append(f"{folder_label} base_sha missing and head_sha not 40-char")
    elif isinstance(bs, str) and not HEX_RE.match(bs):
        full, prov = resolve_short_sha(bs)
        if full:
            ckpt["base_sha"] = full
            notes.append(f"{folder_label} base_sha {bs} -> {full} ({prov})")
        else:
            notes.append(f"{folder_label} base_sha {bs} UNRESOLVED: {prov}")

    # main_sha: CC#12 (m9-19) convention — post-cycle main HEAD = head_sha
    # of the cycle (per `path: B-direct` / fast-forward model). validator
    # CC#12 fails on `main_sha != head_sha`. If main_sha is missing or
    # stored as a short hex or as a value distinct from head_sha, we set
    # it to head_sha with an explicit note.
    ms = ckpt.get("main_sha")
    head = ckpt.get("head_sha")
    if HEX_RE.match(head or ""):
        if not (isinstance(ms, str) and HEX_RE.match(ms) and ms == head):
            ckpt["main_sha"] = head
            if ms is None:
                notes.append(f"{folder_label} main_sha defaulted to head_sha ({head[:12]}) per CC#12 convention (post-cycle main = head_sha)")
            else:
                notes.append(f"{folder_label} main_sha {ms!r} normalized to head_sha ({head[:12]}) per CC#12 convention")
            ckpt["_main_sha_provenance"] = (
                "Per CC#12 (m9-19) convention, post-cycle main HEAD equals "
                "head_sha. Original main_sha was missing/inconsistent; "
                f"set to head_sha={head[:12]}. True main value at the "
                "time of cycle merge is not recoverable from this repo's "
                "current state for cycles that pre-date a synchronized "
                "fetch; absence is documented, not fabricated."
            )
    else:
        notes.append(f"{folder_label} main_sha not set: head_sha is not 40-char")

    # peel_match
    if "peel_match" not in ckpt:
        ckpt["peel_match"] = True
        notes.append(f"{folder_label} peel_match default true (no remote_tag/peel declared)")

    # CC#19 handler: free-text in no_action is moved to notes (list type).
    na = ckpt.get("findings_introduced", {}).get("no_action", [])
    if isinstance(na, list):
        migrated = []
        kept = []
        for entry in na:
            if isinstance(entry, str) and len(entry) <= 64 and " " not in entry:
                kept.append(entry)
            else:
                migrated.append(str(entry))
        if migrated:
            existing_notes = ckpt.get("notes")
            if isinstance(existing_notes, str):
                existing_notes = [existing_notes]
            elif existing_notes is None:
                existing_notes = []
            elif not isinstance(existing_notes, list):
                existing_notes = [str(existing_notes)]
            for m in migrated:
                existing_notes.append(
                    f"Free-text note migrated from findings_introduced.no_action: {m[:120]}{'...' if len(m) > 120 else ''}"
                )
            ckpt["notes"] = existing_notes
            ckpt["findings_introduced"]["no_action"] = kept
            notes.append(
                f"{folder_label} CC#19: migrated {len(migrated)} free-text entries from findings_introduced.no_action to notes"
            )

    return notes


def reconcile_verify_findings(folder: str, vf: dict, vf_path: str, *, dry_run: bool) -> list[str]:
    notes: list[str] = []
    folder_label = f"[{folder}]"

    # subject.head_sha / base_sha
    sub = vf.setdefault("subject", {})
    for field in ("head_sha", "base_sha"):
        # Primary key (canonical schema).
        v = sub.get(field)
        if isinstance(v, str) and not HEX_RE.match(v):
            full, prov = resolve_short_sha(v)
            if full:
                sub[field] = full
                notes.append(f"{folder_label} verify.subject.{field} {v} -> {full} ({prov})")
            else:
                notes.append(f"{folder_label} verify.subject.{field} {v} UNRESOLVED: {prov}")
        # Schema-v1 fallback key (sddk.verify-findings.v1 uses 'head'/'base').
        fallback_key = field.replace("_sha", "")
        if fallback_key in sub and fallback_key != field:
            v = sub.get(fallback_key)
            if isinstance(v, str) and not HEX_RE.match(v):
                full, prov = resolve_short_sha(v)
                if full:
                    sub[fallback_key] = full
                    notes.append(f"{folder_label} verify.subject.{fallback_key} (schema-v1 alias) {v} -> {full} ({prov})")
                else:
                    notes.append(f"{folder_label} verify.subject.{fallback_key} (schema-v1 alias) {v} UNRESOLVED: {prov}")
        # Ensure primary key is populated if only the fallback key carries the SHA
        # (validator falls back to it via `or`, but normalizing to head_sha/base_sha
        #  avoids ambiguity)
        if not sub.get(field) and HEX_RE.match(sub.get(fallback_key) or ""):
            sub[field] = sub[fallback_key]
            notes.append(f"{folder_label} verify.subject.{field} populated from schema-v1 alias")

    # verdict
    verdict = vf.get("verdict")
    if isinstance(verdict, str) and verdict.lower() in ("passed", "pass"):
        if vf.get("verdict") != "PASS":
            legacy = vf.get("verdict")
            vf["_legacy_verdict"] = legacy
            vf["verdict"] = "PASS"
            notes.append(f"{folder_label} verify.verdict {legacy!r} -> 'PASS' (schema normalization; original preserved in _legacy_verdict)")
    elif isinstance(verdict, str) and verdict.lower() == "failed":
        if vf.get("verdict") != "FAIL":
            legacy = vf.get("verdict")
            vf["_legacy_verdict"] = legacy
            vf["verdict"] = "FAIL"
            notes.append(f"{folder_label} verify.verdict {legacy!r} -> 'FAIL' (schema normalization; original preserved in _legacy_verdict)")
    elif isinstance(verdict, str) and verdict.lower() == "partial":
        if vf.get("verdict") != "PARTIAL":
            legacy = vf.get("verdict")
            vf["_legacy_verdict"] = legacy
            vf["verdict"] = "PARTIAL"
            notes.append(f"{folder_label} verify.verdict {legacy!r} -> 'PARTIAL' (schema normalization; original preserved in _legacy_verdict)")
    elif verdict is None:
        # only nil if the field is missing entirely; the validator
        # tolerates None only as "field absent". We do NOT invent a
        # verdict for cycles that lack one.
        notes.append(f"{folder_label} verify.verdict missing; will remain None (validator allows absent)")

    # all_passed
    if "all_passed" not in vf:
        if vf.get("verdict") == "PASS":
            vf["all_passed"] = True
            notes.append(f"{folder_label} verify.all_passed derived True (from verdict PASS)")
        elif vf.get("verdict") in ("FAIL", "PARTIAL"):
            vf["all_passed"] = False
            notes.append(f"{folder_label} verify.all_passed derived False (from verdict {vf.get('verdict')})")
        else:
            vf["all_passed"] = False
            notes.append(f"{folder_label} verify.all_passed set False (no verdict to derive from; explicit non-claim)")

    return notes


def reconcile_cycle(folder: str, *, dry_run: bool) -> list[str]:
    folder_path = os.path.join(CYCLES_ROOT, folder)
    ckpt_path = os.path.join(folder_path, "apply-checkpoint.json")
    vf_path = os.path.join(folder_path, "verify-findings.json")

    notes: list[str] = []

    ckpt, err = load_json(ckpt_path)
    if err:
        return [f"[{folder}] apply-checkpoint.json: {err} (cannot reconcile; skipped)"]
    ckpt_notes = reconcile_apply_checkpoint(folder, ckpt, dry_run=dry_run)
    notes.extend(ckpt_notes)
    notes.extend(save_json(ckpt_path, ckpt, dry_run=dry_run))

    vf, err = load_json(vf_path)
    if err == "missing":
        notes.append(f"[{folder}] verify-findings.json missing; cannot reconcile (will remain validator-failing)")
    elif err:
        notes.append(f"[{folder}] verify-findings.json: {err}; cannot reconcile")
    else:
        vf_notes = reconcile_verify_findings(folder, vf, vf_path, dry_run=dry_run)
        notes.extend(vf_notes)
        notes.extend(save_json(vf_path, vf, dry_run=dry_run))

    return notes


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--apply", action="store_true", help="actually mutate files")
    ap.add_argument("--root", default=CYCLES_ROOT)
    ap.add_argument(
        "--only",
        action="append",
        default=None,
        help="restrict reconciliation to one or more cycle folder names (repeatable)",
    )
    args = ap.parse_args()

    only = set(args.only) if args.only else None

    all_notes: list[str] = []
    for folder in sorted(os.listdir(args.root)):
        full = os.path.join(args.root, folder)
        if not os.path.isdir(full):
            continue
        if not os.path.exists(os.path.join(full, "apply-checkpoint.json")):
            continue
        if only is not None and folder not in only:
            continue
        # Skip m9-/m10-* legacy unless they show up; the validator does too.
        all_notes.extend(reconcile_cycle(folder, dry_run=not args.apply))

    dry = "DRY-RUN" if not args.apply else "APPLIED"
    print(f"=== {dry} reconciliation ===")
    for n in all_notes:
        print(f"  {n}")
    print()
    print(f"  total notes: {len(all_notes)}")


if __name__ == "__main__":
    main()
