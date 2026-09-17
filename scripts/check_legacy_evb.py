#!/usr/bin/env python3
"""REC-C2 legacy EventBus / queue coupling inventory + ratchet.

The convergence from "EventBus is a second source of truth" to
"ExecutionLog is the only authoritative evidence store" is tracked as a
*shrinking* inventory. This script is both halves:

  * `--write` regenerates `legacy-evb-inventory.json` from the current
    production Rust surface (curated `class`/`owner` fields are preserved
    for entries whose key is unchanged).
  * default mode enforces the ratchet:
      1. no new protected use (an occurrence not in the inventory);
      2. no stale waiver (an inventory entry whose occurrence is gone);
      3. the total occurrence count may only go down;
      4. every entry carries a `class` and an `owner`; a `CANONICAL`
         entry with a destructive token fails under `--strict`
         (REC-C2 close mode).

Scanning rules (so the count reflects productive coupling, not noise):
  * only `crates/*/src/**/*.rs`;
  * comments (`//`, `///`, `//!`, and `/* */`) are ignored;
  * string literals are ignored;
  * whole `#[cfg(test)]` regions are ignored;
  * `crates/*/tests/**` and `*/tests.rs` are never scanned.

No third-party dependencies (Python 3.11+).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INVENTORY = ROOT / "legacy-evb-inventory.json"
SCAN_GLOB = "crates/*/src/**/*.rs"

# token id -> (regex, kind). `kind` drives the default classification and
# the `--strict` rule (`destructive` = a read that consumes or evicts the
# buffer, so it can never be the authoritative source).
TOKENS: dict[str, tuple[str, str]] = {
    "event_bus_type": (r"\bEventBus\w*\b", "type"),
    "snapshot": (r"[A-Za-z_]*bus[A-Za-z_]*\.snapshot\s*\(", "destructive"),
    "snapshot_raw": (r"\.snapshot_raw\s*\(", "destructive"),
    "drain_raw_events": (r"\.drain_raw_events\s*\(", "destructive"),
    "drain": (r"[A-Za-z_]*bus[A-Za-z_]*\.drain\s*\(", "destructive"),
    "drain_fired": (r"\.drain_fired\s*\(", "destructive"),
    "read_since": (r"\.read_since\s*\(", "read"),
    "fired_buffer": (r"\bfired_buffer\b", "queue"),
    "dual_push": (r"\bdual_push\b", "seam"),
}

DEFAULT_CLASS = {
    "destructive": "CANONICAL",
    "queue": "CANONICAL",
    "read": "COMPATIBILITY",
    "seam": "COMPATIBILITY",
    "type": "COMPATIBILITY",
}

DEFAULT_OWNER = {
    "event_bus_type": "REC-C2.3",
    "snapshot": "REC-C2.2",
    "snapshot_raw": "REC-C2.2",
    "drain_raw_events": "REC-C2.2",
    "drain": "REC-C2.2",
    "read_since": "REC-C2.2",
    "fired_buffer": "REC-C2.1",
    "drain_fired": "REC-C2.1",
    "dual_push": "REC-C2.2",
}

DEFAULT_REPLACEMENT = {
    "event_bus_type": "delete EventBus; live fan-out is a transport, not evidence",
    "snapshot": "ExecutionLog read; live fan-out only (no authoritative bus snapshot)",
    "snapshot_raw": "ExecutionLog.read_from_seq / projection::build_engine",
    "drain_raw_events": "ExecutionLog.read_from_seq / projection::build_engine",
    "drain": "events_read cursor over ExecutionLog",
    "read_since": "events_read cursor over ExecutionLog",
    "fired_buffer": "TripwireFired evidence appended to ExecutionLog (C2.1)",
    "drain_fired": "read TripwireFired evidence from ExecutionLog (C2.1)",
    "dual_push": "ExecutionLog.append first, then live fan-out",
}


def strip_noise(text: str) -> list[str | None]:
    """Return per-line text with comments and literals blanked.

    A line fully consumed by a comment or literal becomes `None` (skipped);
    otherwise the code portion is returned with `//` remainders removed.

    Handles `///`, `//!`, nested `/* */`, plain strings, raw strings
    (`r"..."`, `r#"..."#`, `br##"..."##`), byte strings, char literals, and
    **line-continued strings** (a quoted string continued with a trailing
    backslash and the next line) — a string left open at end of line stays
    open on the next line. Without that, a token that appears inside a
    multi-line string would be counted as a real use.
    """
    out: list[str | None] = []
    in_block_comment = 0
    in_string = False  # inside a normal/byte string, carried across lines
    in_raw: int | None = None  # open raw string: number of `#` in its delimiter

    for raw in text.splitlines():
        line = raw
        buf: list[str] = []
        i = 0
        n = len(line)
        while i < n:
            if in_block_comment:
                if line.startswith("/*", i):
                    in_block_comment += 1
                    i += 2
                elif line.startswith("*/", i):
                    in_block_comment -= 1
                    i += 2
                else:
                    i += 1
                continue

            if in_raw is not None:
                closer = '"' + "#" * in_raw
                j = line.find(closer, i)
                if j == -1:
                    i = n
                else:
                    i = j + len(closer)
                    in_raw = None
                continue

            if in_string:
                closed = False
                while i < n:
                    if line[i] == "\\":
                        i += 2
                        continue
                    if line[i] == '"':
                        i += 1
                        closed = True
                        break
                    i += 1
                if closed:
                    in_string = False
                buf.append(" ")
                continue

            if line.startswith("//", i):
                break
            if line.startswith("/*", i):
                in_block_comment += 1
                i += 2
                continue
            m = re.match(r'(?:b)?r(#*)"', line[i:])
            if m:
                hashes = m.group(1)
                closer = '"' + hashes
                j = line.find(closer, i + m.end())
                if j == -1:
                    in_raw = len(hashes)
                    i = n
                else:
                    i = j + len(closer)
                buf.append(" ")
                continue
            if line.startswith('b"', i) or line[i] == '"':
                i += 2 if line.startswith('b"', i) else 1
                closed = False
                while i < n:
                    if line[i] == "\\":
                        i += 2
                        continue
                    if line[i] == '"':
                        i += 1
                        closed = True
                        break
                    i += 1
                if not closed:
                    in_string = True
                buf.append(" ")
                continue
            if line[i] == "'" and i + 1 < n:
                k = i + 1
                if line[k] == "\\":
                    k += 2
                else:
                    k += 1
                if k < n and line[k] == "'":
                    i = k + 1
                    buf.append(" ")
                    continue
            buf.append(line[i])
            i += 1
        code = "".join(buf)
        out.append(code if code.strip() else None)
    return out


TEST_ATTR_RE = re.compile(r"^\s*#\[(?:cfg\(test\)|test\]|tokio::test\b)")
MOD_TESTS_RE = re.compile(r"^\s*(?:pub\s+)?mod\s+tests\b")


def cfg_test_lines(lines: list[str | None]) -> set[int]:
    """1-indexed lines that belong to test code (brace-matched).

    Covers three shapes so historical tests are never counted as
    productive coupling:
      * `#[cfg(test)]` regions (nested attributes handled);
      * `mod tests { ... }` even without a cfg gate (this repo has one
        that is intentionally not gated);
      * `#[test]` / `#[tokio::test]` function bodies.
    """
    covered: set[int] = set()
    n = len(lines)
    i = 0
    while i < n:
        code = lines[i]
        is_test_start = code is not None and (
            TEST_ATTR_RE.match(code) or MOD_TESTS_RE.match(code)
        )
        if is_test_start:
            depth = 0
            started = False
            j = i
            while j < n:
                c = lines[j]
                if c is not None:
                    for ch in c:
                        if ch == "{":
                            depth += 1
                            started = True
                        elif ch == "}":
                            depth -= 1
                covered.add(j + 1)
                if started and depth <= 0:
                    break
                j += 1
            i = j + 1
            continue
        i += 1
    return covered


SYMBOL_RE = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:unsafe\s+)?"
    r"(?:fn|struct|enum|trait|impl|mod|type|use|const|static)\s+([A-Za-z_][A-Za-z0-9_]*)"
)
IMPL_RE = re.compile(r"^\s*impl(?:<[^>]*>)?\s+([A-Za-z_][A-Za-z0-9_]*)")


def scan() -> list[dict]:
    """Every production occurrence of a protected token."""
    found: list[dict] = []
    for path in sorted(ROOT.glob(SCAN_GLOB)):
        rel = path.relative_to(ROOT).as_posix()
        if rel.endswith("/tests.rs") or "/tests/" in rel:
            continue
        lines = strip_noise(path.read_text(encoding="utf-8", errors="ignore"))
        skip = cfg_test_lines(lines)
        # Depth-aware symbol anchor: a stack of (depth_at_decl, name).
        stack: list[tuple[int, str]] = []
        depth = 0
        for idx, code in enumerate(lines, start=1):
            if code is None or idx in skip:
                continue
            # Pop symbols whose block has closed.
            while stack and stack[-1][0] > depth:
                stack.pop()
            name = None
            m = SYMBOL_RE.match(code)
            if m:
                name = m.group(1)
            else:
                m = IMPL_RE.match(code)
                if m:
                    name = m.group(1)
            if name is not None:
                stack.append((depth, name))
            symbol = stack[-1][1] if stack else "<top>"
            for token, (pat, kind) in TOKENS.items():
                for _ in re.finditer(pat, code):
                    found.append(
                        {
                            "key": f"{rel}::{symbol}::{token}",
                            "path": rel,
                            "symbol": symbol,
                            "token": token,
                            "kind": kind,
                            "line": idx,
                        }
                    )
            depth += code.count("{") - code.count("}")
    # Disambiguate duplicate keys (same file+symbol+token more than once).
    counts: dict[str, int] = {}
    for occ in found:
        base = occ["key"]
        counts[base] = counts.get(base, 0) + 1
        occ["key"] = f"{base}#{counts[base]}"
    return found


def load_inventory() -> dict:
    if not INVENTORY.exists():
        return {"entries": [], "baseline_total": 0}
    return json.loads(INVENTORY.read_text(encoding="utf-8"))


def write_inventory() -> int:
    old = {e["key"]: e for e in load_inventory().get("entries", [])}
    occ = scan()
    entries = []
    for o in occ:
        prev = old.get(o["key"], {})
        entries.append(
            {
                "key": o["key"],
                "path": o["path"],
                "symbol": o["symbol"],
                "token": o["token"],
                "kind": o["kind"],
                "line": o["line"],
                "class": prev.get("class", DEFAULT_CLASS[o["kind"]]),
                "owner": prev.get("owner", DEFAULT_OWNER[o["token"]]),
                "replacement": prev.get(
                    "replacement", DEFAULT_REPLACEMENT[o["token"]]
                ),
                "removal_gate": prev.get("removal_gate", DEFAULT_OWNER[o["token"]]),
            }
        )
    doc = {
        "schema": "chronos.legacy-evb-inventory.v1",
        "note": (
            "Shrinking inventory of production EventBus / legacy-queue coupling. "
            "Regenerate with `scripts/check_legacy_evb.py --write`; the checker "
            "fails on new uses, stale waivers, and (under --strict) any CANONICAL "
            "destructive read."
        ),
        "baseline_total": len(entries),
        "entries": entries,
    }
    INVENTORY.write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {INVENTORY.relative_to(ROOT)}: {len(entries)} entries")
    return 0


def check(strict: bool) -> int:
    inv = load_inventory()
    if INVENTORY.exists() and not inv.get("entries"):
        # REC-C2.3: empty inventory is the goal state ("ratchet reached 0"),
        # distinct from a missing inventory file (which still requires --write).
        print(
            f"legacy-evb ratchet PASSED (ratchet reached 0: 0 production uses tracked, "
            f"baseline {inv.get('baseline_total', 0)})."
        )
        return 0
    if not inv.get("entries"):
        print("ERROR: missing or empty legacy-evb-inventory.json (run --write)")
        return 1
    entries = inv["entries"]
    accepted = {e["key"]: e for e in entries}
    occ = scan()
    current = {o["key"]: o for o in occ}

    errors: list[str] = []

    for key in current:
        if key not in accepted:
            o = current[key]
            errors.append(
                f"new protected use: {o['path']}:{o['line']} "
                f"[{o['token']}] {o['symbol']} (not in the inventory)"
            )

    for key, e in accepted.items():
        if key not in current:
            errors.append(
                f"stale waiver: {e['path']} [{e['token']}] {e['symbol']} "
                f"declared but no longer present in the code"
            )

    total = len(current)
    baseline = inv.get("baseline_total", len(entries))
    if total > baseline:
        errors.append(
            f"total protected uses grew: {total} > baseline {baseline} "
            f"(the count may only shrink)"
        )

    for e in entries:
        if not e.get("class"):
            errors.append(f"entry {e['key']} has no class")
        if not e.get("owner"):
            errors.append(f"entry {e['key']} has no owner")

    if strict:
        for e in entries:
            if e.get("class") == "CANONICAL" and e["kind"] == "destructive":
                errors.append(
                    f"CANONICAL destructive read still present: "
                    f"{e['path']} [{e['token']}] {e['symbol']} "
                    f"(owner {e.get('owner')})"
                )

    if errors:
        for err in errors:
            print(f"ERROR: {err}")
        print(f"\nlegacy-evb ratchet FAILED with {len(errors)} finding(s).")
        return 1
    print(
        f"legacy-evb ratchet PASSED ({total} production uses tracked, "
        f"baseline {baseline}, "
        f"CANONICAL={sum(1 for e in entries if e.get('class') == 'CANONICAL')})."
    )
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true", help="regenerate the inventory")
    parser.add_argument(
        "--strict",
        action="store_true",
        help="REC-C2 close mode: fail on any CANONICAL destructive read",
    )
    args = parser.parse_args()
    if args.write:
        return write_inventory()
    return check(args.strict)


if __name__ == "__main__":
    sys.exit(main())
