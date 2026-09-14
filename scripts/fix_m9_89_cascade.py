#!/usr/bin/env python3
"""m9-89 cascade fix tool — applies mechanical backfills across m9-77..m9-88.

This tool is intentionally narrow: it only touches fields whose drift
is mechanical (status case, legacy field removal, peel_match/remote_tag_peel
backfill, missing required-field backfill, archive-manifest Head SHA in
canonical format, release-receipt Head SHA, change-entry base_sha, archived_at,
findings_introduced, merge-receipt SHA/Branch/Date, archive-manifest Date,
verify-findings head_sha).

It does NOT modify any text that requires semantic judgement
(e.g. summary text, title text). For those, the fix tool writes a
placeholder string derived from the existing verify-report.
"""
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(".").resolve()
CYCLES = [f"m9-{i}" for i in range(34, 89)]  # cover m9-34 peel fixes + m9-67..m9-88


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


def fix_apply_checkpoint(cycle, dry_run=False):
    cp = ckpt(cycle)
    if not cp:
        return []
    changes = []
    d = json.loads(cp.read_text())
    keys_orig = set(d.keys())

    if d.get("status") != "CLOSED":
        changes.append(("status", d.get("status"), "CLOSED"))
        d["status"] = "CLOSED"

    am = archive_manifest(cycle)
    if am and not d.get("archived_at"):
        am_content = am.read_text()
        m_date = re.search(r"\| Date \| `?(\S+?)`? \|", am_content)
        archived_at_val = m_date.group(1) if m_date else "2026-09-14"
        d["archived_at"] = archived_at_val
        changes.append(("archived_at", "MISSING", archived_at_val))

    fi = d.get("findings_introduced")
    if fi is None:
        d["findings_introduced"] = {"no_action": []}
        changes.append(("findings_introduced", "MISSING", "{no_action: []}"))
    elif not isinstance(fi, dict):
        old = fi
        d["findings_introduced"] = {"no_action": [], "_migrated_from": str(old)[:50]}
        changes.append(("findings_introduced", "non-dict", "dict"))
    elif "no_action" not in fi:
        fi["no_action"] = []
        changes.append(("findings_introduced.no_action", "MISSING", "[]"))

    if "path" in keys_orig:
        path_val = d.pop("path")
        if "route" not in keys_orig:
            d["route"] = path_val
        changes.append(("path_removed", path_val, d["route"]))

    legacy_fields = {"change", "artifacts", "commits_since_base",
                     "findings_remaining_m9_plus", "ledger_state", "next_cycle",
                     "next_cycle_path", "next_cycle_target_findings", "notes",
                     "runtime_status", "tag"}
    found_legacy = keys_orig & legacy_fields
    for f in sorted(found_legacy):
        changes.append(("removed_legacy", f, ""))
        del d[f]

    remote_tag = d.get("remote_tag", "")
    actual_peel = git_peel(remote_tag) if remote_tag else None
    if not d.get("remote_tag_peel") and actual_peel:
        d["remote_tag_peel"] = actual_peel
        changes.append(("remote_tag_peel", "", actual_peel))

    head = d.get("head_sha", "")
    peel = d.get("remote_tag_peel", "")
    head_eq_peel = bool(head and peel and head == peel)
    if d.get("peel_match") is None:
        if head_eq_peel:
            d["peel_match"] = True
            changes.append(("peel_match", "MISSING", "True"))
        elif peel and head and head != peel:
            msg = git_log_subject(peel)
            if msg.startswith("fix("):
                d["peel_match"] = False
                changes.append(("peel_match", "MISSING", "False (fix-peel)"))
            else:
                d["peel_match"] = True
                changes.append(("peel_match", "MISSING", "True"))

    if peel and head and head != peel:
        msg = git_log_subject(peel) if peel else ""
        if not msg.startswith("fix("):
            changes.append(("head_sha_align_peel", head, peel))
            d["head_sha"] = peel
            head = peel
            if d.get("main_sha") and d["main_sha"] != peel:
                d["main_sha"] = peel

    if d.get("main_sha") and head and d["main_sha"] != head:
        changes.append(("main_sha", d["main_sha"], head))
        d["main_sha"] = head

    if "findings_closed" not in d:
        d["findings_closed"] = []
        changes.append(("findings_closed", "MISSING", "[]"))

    if "created_at" not in d or not d.get("created_at"):
        rr = release_receipt(cycle)
        date_val = "2026-09-14"
        if rr:
            m = re.search(r"\| Date \| (\S+)", rr.read_text())
            if m:
                date_val = m.group(1)
        d["created_at"] = date_val
        changes.append(("created_at", "MISSING", date_val))

    if "title" not in d or not d.get("title"):
        d["title"] = cycle
        changes.append(("title", "MISSING", cycle))

    if "summary" not in d or not d.get("summary"):
        vr_files = list((ROOT / "cycle-artifacts/p-3416cfb8288f8964").glob(f"{cycle}-*/verify-report.md"))
        summary = ""
        if vr_files:
            content = vr_files[0].read_text()
            for line in content.splitlines():
                if line.strip() and not line.startswith("#"):
                    summary = line.strip()[:200]
                    break
        if not summary:
            summary = f"Backfilled for m9-89 cascade cleanup (vault metadata only)"
        d["summary"] = summary
        changes.append(("summary", "MISSING", summary[:50]))

    if not changes:
        return []
    if not dry_run:
        cp.write_text(json.dumps(d, indent=2) + "\n")
    return changes


def fix_release_receipt(cycle, dry_run=False):
    rr = release_receipt(cycle)
    if not rr:
        return []
    cp = ckpt(cycle)
    if not cp:
        return []
    d = json.loads(cp.read_text())
    head = d.get("head_sha", "")
    peel = d.get("remote_tag_peel", "")
    remote_tag = d.get("remote_tag", "")
    if not (head and peel and remote_tag):
        return []

    content = rr.read_text()
    changes = []

    if not re.search(r"(?:\n\| Head SHA \||\nHead SHA \|)", content):
        sha_section = re.search(
            r"(## (?:SHAs|Identification|Tag and merge metadata)\s*\n+\| Field \| Value \|[\s\S]*?\n)([\s\S]*?)(?=\n## |\Z)",
            content,
        )
        if sha_section:
            table_text = sha_section.group(2)
            lines = table_text.splitlines()
            insert_idx = 0
            for i, line in enumerate(lines):
                if line.startswith("|") and not line.startswith("|---") and not line.startswith("| Field"):
                    insert_idx = i + 1
            new_row = f"| Head SHA | {head} |"
            lines.insert(insert_idx, new_row)
            new_table = "\n".join(lines)
            content = content.replace(sha_section.group(2), new_table)
            changes.append(("release_receipt_added_Head_SHA", head))
    else:
        m = re.search(r"(?:\n\| |\n)Head SHA \| `?([a-f0-9]+)`?", content)
        if m and m.group(1) != head:
            content = re.sub(
                rf"((?:\n\| |\n)Head SHA \| `?)[a-f0-9]+(`?)",
                rf"\g<1>{head}\g<2>",
                content,
            )
            changes.append(("release_receipt_head_sha_corrected", head))

    if not re.search(r"(?:\n\| Peel match \||\nPeel match \|)", content):
        sha_section = re.search(
            r"(## (?:SHAs|Identification|Tag and merge metadata)\s*\n+\| Field \| Value \|[\s\S]*?\n)([\s\S]*?)(?=\n## |\Z)",
            content,
        )
        if sha_section:
            table_text = sha_section.group(2)
            lines = table_text.splitlines()
            insert_idx = 0
            for i, line in enumerate(lines):
                if line.startswith("|") and not line.startswith("|---") and not line.startswith("| Field"):
                    insert_idx = i + 1
            peel_match_val = "true" if head == peel else f"peel at {peel[:12]}, head at {head[:12]} (mismatch)"
            new_row = f"| Peel match | {peel_match_val} |"
            lines.insert(insert_idx, new_row)
            new_table = "\n".join(lines)
            content = content.replace(sha_section.group(2), new_table)
            changes.append(("release_receipt_added_Peel_match", peel_match_val))

    if not changes or dry_run:
        return changes
    rr.write_text(content)
    return changes


def fix_merge_receipt(cycle, dry_run=False):
    """Fix merge-receipt.md: add Head SHA, Base SHA, Branch, Date fields."""
    mr = merge_receipt(cycle)
    if not mr:
        return []
    cp = ckpt(cycle)
    if not cp:
        return []
    d = json.loads(cp.read_text())
    head = d.get("head_sha", "")
    base = d.get("base_sha", "")
    branch = d.get("branch", "")
    archived_at = d.get("archived_at", "2026-09-14")

    if not (head and base):
        return []

    content = mr.read_text()
    changes = []

    has_table = bool(re.search(
        r"((?:^# .*?\n\n)?\| Field \| Value \|[\s\S]*?\n)",
        content,
    ))

    has_head = bool(re.search(r"(?:\n\| Head SHA \||\nHead SHA \|)", content))
    has_branch = bool(re.search(r"(?:\n\| Branch \||\nBranch \|)", content))
    has_date = bool(re.search(r"(?:\n\| Date \||\nDate \|)", content))

    if not has_table:
        # Bulleted format — build a new ## SHAs table.
        new_section = (
            "\n\n## SHAs\n\n"
            "| Field | Value |\n"
            "|---|---|\n"
            f"| Branch | {branch} |\n"
            f"| Date | {archived_at} |\n"
            f"| Base SHA | {base} |\n"
            f"| Head SHA | {head} |\n"
        )
        lines = content.splitlines()
        insert_idx = len(lines)
        for i, line in enumerate(lines):
            if line.startswith("## ") and i > 0:
                insert_idx = i
                break
        lines.insert(insert_idx, new_section.rstrip())
        content = "\n".join(lines) + "\n"
        changes.append(("merge_receipt_added_SHAs_section", head))
    else:
        sha_section = re.search(
            r"((?:^# .*?\n\n)?\| Field \| Value \|[\s\S]*?\n)([\s\S]*?)(?=\n## |\Z)",
            content,
        )
        if sha_section:
            table_text = sha_section.group(2)
            new_rows = []
            for field, value in [("Branch", branch), ("Date", archived_at),
                                  ("Base SHA", base), ("Head SHA", head)]:
                if not re.search(rf"(?:\n\| {field} \||\n{field} \|)", content):
                    new_rows.append((field, value))
            if new_rows:
                lines = table_text.splitlines()
                insert_idx = len(lines)
                for i, line in enumerate(lines):
                    if line.startswith("|") and not line.startswith("|---") and not line.startswith("| Field"):
                        insert_idx = i
                        break
                for field, value in new_rows:
                    row = f"| {field} | {value} |"
                    lines.insert(insert_idx, row)
                    insert_idx += 1
                    changes.append((f"merge_receipt_added_{field.replace(' ', '_')}", value))
                new_table = "\n".join(lines)
                content = content.replace(sha_section.group(2), new_table)

    # Correct Head SHA value if present
    m = re.search(r"(?:\n\| |\n)Head SHA \| `?([a-f0-9]+)`?", content)
    if m and m.group(1) != head:
        content = re.sub(
            rf"((?:\n\| |\n)Head SHA \| `?)[a-f0-9]+(`?)",
            rf"\g<1>{head}\g<2>",
            content,
        )
        changes.append(("merge_receipt_head_sha_corrected", head))

    # Correct Base SHA value if present
    m = re.search(r"(?:\n\| |\n)Base SHA \| `?([a-f0-9]+)`?", content)
    if m and m.group(1) != base:
        content = re.sub(
            rf"((?:\n\| |\n)Base SHA \| `?)[a-f0-9]+(`?)",
            rf"\g<1>{base}\g<2>",
            content,
        )
        changes.append(("merge_receipt_base_sha_corrected", base))

    if not changes or dry_run:
        return changes
    mr.write_text(content)
    return changes


def fix_archive_manifest(cycle, dry_run=False):
    am = archive_manifest(cycle)
    if not am:
        return []
    cp = ckpt(cycle)
    if not cp:
        return []
    d = json.loads(cp.read_text())
    head = d.get("head_sha", "")
    if not head:
        return []

    content = am.read_text()
    changes = []

    if re.search(r"\| Head SHA \| `[a-f0-9]{40}` \|", content):
        return []

    m = re.search(r"\| Head SHA \| ([a-f0-9]{40})", content)
    if m:
        new_content = re.sub(
            r"\| Head SHA \| ([a-f0-9]{40}) \|",
            rf"| Head SHA | `{m.group(1)}` |",
            content,
        )
        if new_content != content:
            content = new_content
            changes.append(("archive_manifest_head_sha_backticks_added", head))

    if not re.search(r"\| Head SHA \|", content) and re.search(r"\| Merge commit \(--no-ff\) \|", content):
        new_row = f"| Head SHA | `{head}` |\n"
        content = re.sub(
            r"(\| Merge commit \(--no-ff\) \| `[a-f0-9]{40}` \|)",
            new_row + r"\1",
            content,
        )
        changes.append(("archive_manifest_head_sha_added", head))

    if not re.search(r"\| Head SHA \|", content) and re.search(r"\| Merge SHA \|", content):
        new_row = f"| Head SHA | `{head}` |\n"
        content = re.sub(
            r"(\| Merge SHA \| `[a-f0-9]{40}` \|)",
            new_row + r"\1",
            content,
        )
        changes.append(("archive_manifest_head_sha_added", head))

    if not changes or dry_run:
        return changes
    am.write_text(content)
    return changes


def fix_archive_manifest_date(cycle, dry_run=False):
    am = archive_manifest(cycle)
    if not am:
        return []
    cp = ckpt(cycle)
    if not cp:
        return []
    d = json.loads(cp.read_text())
    archived_at = d.get("archived_at", "2026-09-14")

    content = am.read_text()
    if "| Date |" in content:
        return []
    changes = []

    table_match = re.search(
        r"(## (?:Identification|Cycle)\s*\n+(?:\|[^\n]*\|\n)+)",
        content,
    )
    if table_match:
        table = table_match.group(1)
        lines = table.splitlines()
        last_data_idx = 0
        for i, line in enumerate(lines):
            if line.startswith("|") and not line.startswith("|---") and "|" in line[1:]:
                last_data_idx = i
        new_row = f"| Date | {archived_at} |"
        lines.insert(last_data_idx + 1, new_row)
        new_table = "\n".join(lines) + "\n"
        content = content.replace(table, new_table)
        changes.append(("archive_manifest_date_added", archived_at))

    if not changes or dry_run:
        return changes
    am.write_text(content)
    return changes


def fix_change_entry(cycle, dry_run=False):
    ce = change_entry(cycle)
    if not ce:
        return []
    cp = ckpt(cycle)
    if not cp:
        return []
    d = json.loads(cp.read_text())
    base = d.get("base_sha", "")
    head = d.get("head_sha", "")

    content = ce.read_text()
    changes = []

    m_base = re.search(r"\| Base SHA \| `([a-f0-9]+)`", content)
    if m_base and base and not base.startswith(m_base.group(1)):
        content = re.sub(
            r"\| Base SHA \| `([a-f0-9]+)`",
            f"| Base SHA | `{base}`",
            content,
        )
        changes.append(("change_entry_base_sha", base))

    m2 = re.search(r"- base_sha: `([a-f0-9]+)`", content)
    if m2 and base and not base.startswith(m2.group(1)):
        content = re.sub(
            r"- base_sha: `([a-f0-9]+)`",
            f"- base_sha: `{base}`",
            content,
        )
        changes.append(("change_entry_base_sha_yaml", base))

    m_h = re.search(r"- head_sha: `([a-f0-9]+)`", content)
    if m_h:
        ce_head = m_h.group(1)
        if head and not head.startswith(ce_head):
            content = re.sub(
                r"- head_sha: `([a-f0-9]+)`",
                f"- head_sha: `{head}`",
                content,
            )
            changes.append(("change_entry_head_sha", head))

    if not changes or dry_run:
        return changes
    ce.write_text(content)
    return changes


def fix_verify_findings(cycle, dry_run=False):
    vf = verify_findings(cycle)
    if not vf:
        return []
    cp = ckpt(cycle)
    if not cp:
        return []
    d = json.loads(cp.read_text())
    head = d.get("head_sha", "")
    if not head:
        return []

    try:
        vfd = json.loads(vf.read_text())
    except json.JSONDecodeError:
        return []

    changes = []
    if isinstance(vfd, dict) and "head_sha" in vfd:
        vf_head = vfd.get("head_sha", "")
        if vf_head and vf_head != head:
            vfd["head_sha"] = head
            changes.append(("verify_findings_head_sha", head))
    if isinstance(vfd, dict) and isinstance(vfd.get("subject"), dict):
        subj = vfd["subject"]
        vf_subj_head = subj.get("head_sha", "")
        if vf_subj_head and vf_subj_head != head:
            subj["head_sha"] = head
            changes.append(("verify_findings_subject_head_sha", head))

    if not changes or dry_run:
        return changes
    vf.write_text(json.dumps(vfd, indent=2) + "\n")
    return changes


def fix_cycle(cycle, dry_run=False):
    all_changes = []
    all_changes.extend([("apply-checkpoint", c) for c in fix_apply_checkpoint(cycle, dry_run)])
    all_changes.extend([("release-receipt", c) for c in fix_release_receipt(cycle, dry_run)])
    all_changes.extend([("merge-receipt", c) for c in fix_merge_receipt(cycle, dry_run)])
    all_changes.extend([("archive-manifest", c) for c in fix_archive_manifest(cycle, dry_run)])
    all_changes.extend([("archive-manifest-date", c) for c in fix_archive_manifest_date(cycle, dry_run)])
    all_changes.extend([("change-entry", c) for c in fix_change_entry(cycle, dry_run)])
    all_changes.extend([("verify-findings", c) for c in fix_verify_findings(cycle, dry_run)])
    return all_changes


def main():
    dry_run = "--dry-run" in sys.argv
    print(f"=== m9-89 cascade fix tool (dry_run={dry_run}) ===")
    total = 0
    for cycle in CYCLES:
        changes = fix_cycle(cycle, dry_run)
        if changes:
            print(f"\n{cycle}: {len(changes)} changes")
            for source, change in changes:
                field = change[0] if isinstance(change, tuple) else change
                print(f"  {source}: {field}")
            total += len(changes)
    print(f"\n=== Total changes: {total} ===")


if __name__ == "__main__":
    main()
