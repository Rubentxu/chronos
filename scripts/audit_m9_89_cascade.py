#!/usr/bin/env python3
"""m9-89 cascade audit — what exactly needs fixing for each CC drift line."""
import json
import os
import subprocess
import re
import sys
from pathlib import Path

ROOT = Path(".").resolve()
CYCLES = [f"m9-{i}" for i in range(67)] + [f"m9-{i}" for i in range(77, 89)]


def ckpt(cycle):
    folder = list((ROOT / f"cycle-artifacts/p-3416cfb8288f8964").glob(f"{cycle}-*/apply-checkpoint.json"))
    return folder[0] if folder else None


def release_receipt(cycle):
    folder = list((ROOT / f"cycle-artifacts/p-3416cfb8288f8964").glob(f"{cycle}-*/release-receipt.md"))
    return folder[0] if folder else None


def merge_receipt(cycle):
    folder = list((ROOT / f"cycle-artifacts/p-3416cfb8288f8964").glob(f"{cycle}-*/merge-receipt.md"))
    return folder[0] if folder else None


def archive_manifest(cycle):
    folder = list((ROOT / f".sddk-knowledge/p-3416cfb8288f8964/changes/archive").glob(f"{cycle}-*/archive-manifest.md"))
    return folder[0] if folder else None


def verify_findings(cycle):
    folder = list((ROOT / f"cycle-artifacts/p-3416cfb8288f8964").glob(f"{cycle}-*/verify-findings.json"))
    return folder[0] if folder else None


def change_entry(cycle):
    folder = list((ROOT / f".sddk-knowledge/p-3416cfb8288f8964/changes").glob(f"{cycle}-*/change-entry.md"))
    return folder[0] if folder else None


def git_peel(tag):
    try:
        return subprocess.check_output(
            ["git", "rev-list", "-n", "1", tag], stderr=subprocess.DEVNULL, cwd=ROOT
        ).decode().strip()
    except subprocess.CalledProcessError:
        return None


def git_exists(sha):
    try:
        subprocess.check_output(["git", "cat-file", "-e", sha], stderr=subprocess.DEVNULL, cwd=ROOT)
        return True
    except subprocess.CalledProcessError:
        return False


def git_log_subject(sha):
    try:
        return subprocess.check_output(
            ["git", "log", "-n", "1", "--format=%s", sha], stderr=subprocess.DEVNULL, cwd=ROOT
        ).decode().strip()
    except subprocess.CalledProcessError:
        return ""


def main():
    audit = {}
    for cycle in CYCLES:
        cp = ckpt(cycle)
        if not cp:
            audit[cycle] = {"missing_apply_checkpoint": True}
            continue
        d = json.loads(cp.read_text())
        fixes = {}
        head = d.get("head_sha", "")
        base = d.get("base_sha", "")
        main_sha = d.get("main_sha", "")
        peel = d.get("remote_tag_peel", "")
        peel_match = d.get("peel_match", None)
        remote_tag = d.get("remote_tag", "")
        status = d.get("status", "")
        keys = set(d.keys())

        if head and peel and head != peel:
            msg = git_log_subject(peel) if peel else ""
            if not msg.startswith("fix("):
                fixes["cc3_head_align_peel"] = {"old": head, "new": peel}

        ce = change_entry(cycle)
        if ce and base:
            content = ce.read_text()
            m_base = re.search(r"\| Base SHA \| `([a-f0-9]+)`", content)
            if m_base and not base.startswith(m_base.group(1)):
                fixes["cc7_change_entry_base_sha"] = {"old": m_base.group(1), "new": base}

        for field in ("head_sha", "base_sha", "main_sha", "remote_tag_peel"):
            v = d.get(field, "")
            if v and not git_exists(v):
                fixes[f"cc8_ckpt_{field}_not_in_git"] = v

        am = archive_manifest(cycle)
        if am:
            content = am.read_text()
            m_head = re.search(r"\| Head SHA \| `([a-f0-9]{40})`", content)
            if not m_head:
                fixes["cc8_archive_manifest_missing_head_sha"] = True

        if status != "CLOSED":
            fixes["cc11_status"] = status
        if am and not d.get("archived_at"):
            fixes["cc11_archived_at"] = "MISSING"
        fi = d.get("findings_introduced")
        if fi is None:
            fixes["cc11_findings_introduced"] = "MISSING"
        elif not isinstance(fi, dict):
            fixes["cc11_findings_introduced"] = "non-dict"
        elif "no_action" not in fi:
            fixes["cc11_findings_introduced"] = "no_action MISSING"

        if main_sha and head and main_sha != head:
            fixes["cc12_main_sha"] = {"old": main_sha, "new": head}

        legacy_fields = {"change", "artifacts", "commits_since_base",
                         "findings_remaining_m9_plus", "ledger_state", "next_cycle",
                         "next_cycle_path", "next_cycle_target_findings", "notes",
                         "runtime_status", "tag"}
        found_legacy = keys & legacy_fields
        if found_legacy:
            fixes["cc14_legacy_fields"] = sorted(found_legacy)

        for f in ("created_at", "title", "summary"):
            if f not in d or not d.get(f):
                fixes[f"cc15_{f}"] = True

        rr = release_receipt(cycle)
        if rr:
            content = rr.read_text()
            for field in ("Head SHA", "Remote tag", "Remote tag_peel", "Peel match"):
                if not re.search(rf"(?:\n\| {field} \||\n{field} \|)", content):
                    fixes[f"cc22_release_receipt_missing_{field}"] = True
            h_m = re.search(r"Head SHA \| ([a-f0-9]+)", content)
            if h_m and head and h_m.group(1) != head:
                fixes["cc43_release_receipt_head_sha"] = {"old": h_m.group(1), "new": head}

        mr = merge_receipt(cycle)
        if mr:
            content = mr.read_text()
            h_m = re.search(r"Head SHA \| ([a-f0-9]+)", content)
            if h_m and head and h_m.group(1) != head:
                fixes["cc23_merge_receipt_head_sha"] = {"old": h_m.group(1), "new": head}
            elif not h_m and head:
                fixes["cc23_merge_receipt_missing_Head_SHA"] = head
            b_m = re.search(r"Base SHA \| ([a-f0-9]+)", content)
            if b_m and base and b_m.group(1) != base:
                fixes["cc23_merge_receipt_base_sha"] = {"old": b_m.group(1), "new": base}
            elif not b_m and base:
                fixes["cc23_merge_receipt_missing_Base_SHA"] = base

        if "path" in keys:
            fixes["cc29_path_field"] = True

        if ce and head:
            content = ce.read_text()
            m_h = re.search(r"- head_sha: `([a-f0-9]+)`", content)
            if m_h and not head.startswith(m_h.group(1)):
                fixes["cc29_change_entry_head_sha"] = {"old": m_h.group(1), "new": head}

        if "findings_closed" not in d:
            fixes["cc40_findings_closed"] = True

        if am:
            content = am.read_text()
            if "| Date |" not in content:
                fixes["cc40_archive_manifest_date"] = True

        vf = verify_findings(cycle)
        if vf:
            try:
                vfd = json.loads(vf.read_text())
            except json.JSONDecodeError:
                vfd = None
            vf_head = ""
            if isinstance(vfd, dict):
                subj = vfd.get("subject", None)
                if isinstance(subj, dict):
                    vf_head = subj.get("head_sha", "")
                elif "head_sha" in vfd:
                    vf_head = vfd.get("head_sha", "")
            if vf_head and head and vf_head != head:
                fixes["cc43_verify_findings_head_sha"] = {"old": vf_head[:8], "new": head[:8]}

        if fixes:
            audit[cycle] = fixes

    print(json.dumps(audit, indent=2, sort_keys=True))
    total = sum(len(v) for v in audit.values())
    print(f"\n# Total fixes: {total} across {len(audit)} cycles", file=sys.stderr)


if __name__ == "__main__":
    main()
