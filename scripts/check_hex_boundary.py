#!/usr/bin/env python3
"""Chronos hexagonal boundary check (REC-C3.1).

This checker is the mechanical half of contract HEX-001. It enforces
that the `chronos_domain::ports` surface is only consumed through
the abstract traits, not via leaked concrete adapters from the
infrastructure crates.

Two rule sets:

  1. **Outbound purity**: modules under `crates/chronos-domain/src/ports/`
     must only `use` other modules inside `chronos_domain`. Any import
     of `chronos_log::*`, `chronos_native::*`, `chronos_capture::*`,
     `chronos_ebpf::*`, or any other infrastructure crate is a HEX
     violation.

  2. **Surface shape**: the public re-export surface declared from
     `ports/mod.rs` is restricted to the listed ports only. Adding a
     new public symbol that is not listed in
     `_EXPECTED_PUBLIC_SYMBOLS` is surfaced as a `note` (not an error
     during C3.1; flipped to error in C3.2 once we settle the
     surface).

Use `python3 scripts/check_hex_boundary.py --check` from CI. Exit code
0 means clean; non-zero lists each violation.

This script is intentionally self-contained: no third-party deps,
reads the Rust source as text. The shape will be re-used (and
extended) by C3.3 when the boundary check becomes a gate on
`chronos_services` as well.
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PORTS_DIR = ROOT / "crates" / "chronos-domain" / "src" / "ports"
MOD_RS = PORTS_DIR / "mod.rs"

INFRA_CRATES = (
    "chronos-log",
    "chronos_capture",
    "chronos_native",
    "chronos_ebpf",
    "chronos_browser",
    "chronos_services",
    "chronos_store",
    "chronos_query",
    "chronos_index",
    "chronos_mcp",
    "chronos_python",
    "chronos_go",
    "chronos_js",
    "chronos_java",
    "chronos_webhook",
    "chronos_cli",
)

# Symbols that the ports surface is supposed to expose after C3.1
# lands. Anything that is `pub use`'d outside this list is a surface
# drift.
_EXPECTED_PUBLIC_SYMBOLS = {
    # execution_log (placeholder)
    "ExecutionLogProviderShape",
    "NoopExecutionLogProvider",
    # notification (pre-existing)
    "NotificationRequest",
    "NotificationTarget",
    "NotificationDeliveryError",
    "NotificationSink",
    "NullNotificationSink",
    # probe (C3.1)
    "ProbeController",
    "ProbeFactory",
    "ProbeRegistry",
    "NullProbeFactory",
    "NullProbeRegistry",
    # session (C3.1)
    "SessionRepository",
    "SessionHandle",
    "SessionState",
    "InMemorySessionRepository",
    # telemetry (C3.1)
    "TelemetryReceiver",
    "TelemetryError",
    "Metric",
    "Counters",
    "NoopTelemetry",
    "InMemoryTelemetry",
}


def _crate_token(crate: str) -> str:
    """Convert `chronos-log` to the `chronos_log` identifier used in
    Rust `use` statements (Cargo normalizes dash to underscore)."""
    return crate.replace("-", "_")


def _scan_ports_modules(errors: list[str]) -> None:
    """Check that no module under `ports/` uses an infra crate."""
    if not PORTS_DIR.is_dir():
        # Ports module is new in C3.1; if missing, that's a regression.
        errors.append(f"missing ports/ directory: {PORTS_DIR}")
        return

    use_pattern = re.compile(r"^\s*use\s+(?P<path>[A-Za-z0-9_:]+)")
    crate_set = {_crate_token(c) for c in INFRA_CRATES}

    for rs in sorted(PORTS_DIR.glob("*.rs")):
        try:
            text = rs.read_text(encoding="utf-8")
        except OSError as exc:
            errors.append(f"{rs.relative_to(ROOT)}: cannot read ({exc})")
            continue

        for lineno, line in enumerate(text.splitlines(), start=1):
            match = use_pattern.match(line)
            if not match:
                continue
            path = match.group("path")
            first = path.split("::", 1)[0]
            if first in crate_set:
                errors.append(
                    f"{rs.relative_to(ROOT)}:{lineno}: ports/ imports "
                    f"infra crate '{first}' (must stay within chronos_domain)"
                )


def _scan_public_surface(errors: list[str], notes: list[str]) -> None:
    """Inspect `pub use ...` blocks in `ports/mod.rs`. Anything not in
    the expected list is a surface drift note.

    We concatenate `pub use` continuations across lines so that
    multi-line `pub use { a, b, c }` blocks are scanned properly.
    Items *missing* from the surface that are expected raise an
    `error`; extra items raise a `note`.
    """
    if not MOD_RS.is_file():
        errors.append(f"missing ports/mod.rs: {MOD_RS}")
        return

    text = MOD_RS.read_text(encoding="utf-8")
    exposed: set[str] = set()

    # Strip line comments so symbols embedded in `// foo::Bar` are not
    # counted. Then look for `pub use ...;` blocks across lines.
    cleaned = re.sub(r"//[^\n]*", "", text)
    for block_match in re.finditer(
        r"pub\s+use\s+(?P<body>[^;]+);", cleaned, flags=re.MULTILINE | re.DOTALL
    ):
        rhs = block_match.group("body")
        for ident in re.findall(r"([A-Z][A-Za-z0-9_]*)", rhs):
            exposed.add(ident)

    missing = sorted(_EXPECTED_PUBLIC_SYMBOLS - exposed)
    extra = sorted(exposed - _EXPECTED_PUBLIC_SYMBOLS)

    for symbol in missing:
        errors.append(
            f"ports/mod.rs does not re-export expected port symbol '{symbol}'"
        )

    if extra:
        notes.append(
            "ports/mod.rs re-exports symbols outside the expected "
            f"surface: {', '.join(extra)}"
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="reserved for future CI use (currently a no-op alias)",
    )
    args = parser.parse_args()

    errors: list[str] = []
    notes: list[str] = []

    _scan_ports_modules(errors)
    _scan_public_surface(errors, notes)

    for note in notes:
        print(f"NOTE: {note}")

    if errors:
        for err in errors:
            print(f"ERROR: {err}")
        return 1

    print("OK: chronos_domain::ports boundary clean.")
    if notes:
        print(f"  ({len(notes)} note(s); see above)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())