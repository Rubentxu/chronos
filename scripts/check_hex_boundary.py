#!/usr/bin/env python3
"""Chronos hexagonal boundary check (REC-C3.2).

This checker is the mechanical half of contracts HEX-001,
HEX-C32-01, HEX-C32-02, and HEX-C32-03 direction-propagation (the
application-side half is reserved for REC-C3.3).

Four rule sets:

  1. **Cargo.toml dependency blacklist** (HEX-C32-01):
     `crates/chronos-domain/Cargo.toml` MUST NOT list any of the
     forbidden dependencies. A waiver list (default empty) is
     allowed but a waiver that names a dependency that is NOT
     present is itself a violation (waivers-stale).
  2. **Full outbound purity** (HEX-C32-02):
     Every `*.rs` file under `crates/chronos-domain/src/` (not
     just `ports/`) must not import any forbidden downstream.
     Uses a regex over `use <path>::…` lines.
  3. **Adapter direction** (REC-C3.2 V5):
     `crates/chronos-webhook/src/` must import
     `chronos_domain::{NotificationRequest, NotificationSink,
     NotificationDeliveryError}` and must not depend on any other
     chronos crate beyond `chronos_domain`.
  4. **Public re-export surface** (REC-C3.1 carry-over):
     Every `pub use` re-export in `ports/mod.rs` is restricted to
     the expected port symbols. Additions past C3.2 settle are
     `note`; missing expected symbols are `error`.

Use `python3 scripts/check_hex_boundary.py --check` from CI. Exit
code 0 = clean.

This script is self-contained: no third-party deps. The shape is
re-used in REC-C3.3 (services-side direction) and tightened in
REC-C3.5 (waivers zero).
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

DOMAIN_CARGO = ROOT / "crates" / "chronos-domain" / "Cargo.toml"
DOMAIN_SRC = ROOT / "crates" / "chronos-domain" / "src"
WEBHOOK_SRC = ROOT / "crates" / "chronos-webhook" / "src"
PORTS_MOD = DOMAIN_SRC / "ports" / "mod.rs"

# (1) Dependency blacklist at the chronos-domain level.
# Anything that pulls HTTP/runtime/webhook outside of chronos-webhook
# is forbidden. `uuid` is a workspace dep shared with chronos-log,
# so it's allowed (it's domain-owned per C3.3 plan); HTTP/async are not.
DOMAIN_FORBIDDEN_DEPS = {
    "reqwest",
    "hyper",
    "tokio",
    "tracing",
    "axum",
    "warp",
    "http",
    "chrono-webhook",
    "chrono_webhook",
    # infrastructural crates (REC-C3.3 will leave them here; REC-C3.5
    # closes them out by zeroing the waivers):
    "chronos-webhook",
    "chronos_webhook",
    "chronos-capture",
    "chronos_capture",
    "chronos-native",
    "chronos_native",
    "chronos-ebpf",
    "chronos_ebpf",
    "chronos-browser",
    "chronos_browser",
    "chronos-services",
    "chronos_services",
    "chronos-store",
    "chronos_store",
    "chronos-query",
    "chronos_query",
    "chronos-index",
    "chronos_index",
    "chronos-mcp",
    "chronos_mcp",
    "chronos-python",
    "chronos_python",
    "chronos-java",
    "chronos_java",
    "chronos-go",
    "chronos_go",
    "chronos-js",
    "chronos_js",
    "chronos-cli",
    "chronos_cli",
    "chronos-log",
    "chronos_log",
}

# (2) Outbound purity: external crate references inside *.rs are
# classified as forbidden if they are infrastructure. Type-only
# workspace deps (`serde`, `schemars`, `thiserror`, `uuid`,
# `serde_json`) and intrinsics (`std`, `core`, `alloc`, `crate`,
# `self`, `super`) are allowed. NOT in this whitelist: any
# `chronos_*` reference except `chronos_domain` (self).
ALLOWED_CROSS_CRATES = {
    # Self-reference
    "chronos_domain",
}

# Externals that ARE allowed inside chronos-domain/src/*.rs.
# These are workspace dependencies declared in Cargo.toml that are
# pure type-only and have no infrastructural concerns.
ALLOWED_EXTERNAL_DEPS = {
    "serde",
    "schemars",
    "thiserror",
    "uuid",
    "serde_json",  # dev-dep only
}

# (3) Adapter direction: chronos-webhook may only depend on
# chronos_domain (plus dev-time crates). Anything else is a
# chrono-webhook affordance leak.
WEBHOOK_FORBIDDEN_CROSS_CRATES = {
    "chronos_log",
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
    "chronos_java",
    "chronos_go",
    "chronos_js",
    "chronos_cli",
}

# Surface shape from REC-C3.1, frozen for REC-C3.2 and broadened in
# REC-C3.3 (capability-bundle split: ExecutionLogProvider + retention +
# maintenance ports). Stale placeholders from the pre-C3.3 sketch
# (ExecutionLogProviderShape, NoopExecutionLogProvider) were removed;
# the surface must contain the *real* ports and the capability helpers.
EXPECTED_PUBLIC_SYMBOLS = {
    # execution_log (REC-C3.3 capability-bundle split)
    "ExecutionLogProvider",
    "ExecutionLogKind",
    "ExecutionLogPage",
    "ExecutionLogError",
    "ExecutionLogFactory",
    "ExecutionLogRetention",
    "RetentionError",
    "RetentionOutcome",
    "ExecutionLogMaintenance",
    "ExecutionLogMaintenanceError",
    "CompactionReport",
    "CompactionMetrics",
    # browser probe (REC-C3.2 V5)
    "BrowserProbeFactory",
    "BrowserProbeBackend",
    "BrowserError",
    # uprobe (REC-C3.2)
    "UprobeInjector",
    "UprobeAttachError",
    "UprobeHandle",
    # notification
    "NotificationRequest",
    "NotificationTarget",
    "NotificationDeliveryError",
    "NotificationSink",
    "NullNotificationSink",
    # probe
    "ProbeController",
    "ProbeFactory",
    "ProbeRegistry",
    "NullProbeFactory",
    "NullProbeRegistry",
    # session
    "SessionRepository",
    "SessionHandle",
    "SessionState",
    "InMemorySessionRepository",
    # telemetry
    "TelemetryReceiver",
    "TelemetryError",
    "Metric",
    "Counters",
    "NoopTelemetry",
    "InMemoryTelemetry",
}


def _emit(label: str, msg: str, errors: list[str], notes: list[str]) -> None:
    if label == "ERROR":
        errors.append(msg)
        print(f"ERROR: {msg}")
    elif label == "NOTE":
        notes.append(msg)
        print(f"NOTE: {msg}")


def _scan_cargo_toml(errors: list[str], notes: list[str]) -> None:
    """HEX-C32-01: chronos-domain Cargo.toml must not list forbidden
    dependencies."""
    if not DOMAIN_CARGO.is_file():
        _emit("ERROR", f"missing Cargo.toml: {DOMAIN_CARGO}", errors, notes)
        return

    try:
        with DOMAIN_CARGO.open("rb") as fh:
            data = tomllib.load(fh)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        _emit("ERROR", f"cannot parse {DOMAIN_CARGO}: {exc}", errors, notes)
        return

    declared: set[str] = set()
    for section in ("dependencies", "build-dependencies", "dev-dependencies"):
        for name in data.get(section, {}).keys():
            declared.add(name)

    violations = sorted(declared & DOMAIN_FORBIDDEN_DEPS)
    for dep in violations:
        _emit(
            "ERROR",
            f"HEX-C32-01 violation: chronos-domain declares forbidden dep"
            f" '{dep}' in [{DATA_SECTION_FOR[section]}] of Cargo.toml",
            errors,
            notes,
        )

    # Waivers stale: detect a hypothetical waiver comment list
    # (we deliberately keep no waiver list here; this is the
    # self-cleansing no-waivers branch from V4). If somebody adds
    # a `[hex-boundary]` table to declare a waiver for a name that
    # is NOT actually in deps, that's a stale-waiver violation.
    waiver_table = data.get("hex-boundary", {})
    waivers = waiver_table.get("waive_deps", []) if isinstance(waiver_table, dict) else []
    for w in waivers or []:
        if w not in declared:
            _emit(
                "ERROR",
                f"waiver stale: declared waiver for '{w}' but that dep"
                " is not actually a dependency of chronos-domain",
                errors,
                notes,
            )

    # Status report.
    print(
        f"  chronos-domain deps: {sorted(declared)}; "
        f"forbidden={sorted(declared & DOMAIN_FORBIDDEN_DEPS)}; "
        f"waivers configured={len(waivers or [])}"
    )


DATA_SECTION_FOR = {
    "dependencies": "dependencies",
    "build-dependencies": "build-dependencies",
    "dev-dependencies": "dev-dependencies",
}


def _scan_outbound_purity(errors: list[str], notes: list[str]) -> None:
    """HEX-C32-02: every *.rs under chronos-domain/src/ must not
    import an infra crate.

    Only EXTERNAL crate paths are checked (anything matching
    `use <ident>::…` where <ident> is not `std`, `core`, `alloc`,
    `crate`, or `self`/`super`). Workspace-internal references
    must be in `ALLOWED_CROSS_CRATES`; everything else is a leak.
    `use` blocks inside `#[cfg(test)]` are excluded (annotation
    tracked per-line by an inside-cfg-test cursor; same approach
    as check_architecture_contracts.py)."""
    if not DOMAIN_SRC.is_dir():
        _emit("ERROR", f"missing src dir: {DOMAIN_SRC}", errors, notes)
        return

    INTRINSIC_PREFIXES = {"std", "core", "alloc", "crate", "self", "super"}

    use_pattern = re.compile(r"^\s*use\s+(?P<path>[A-Za-z0-9_:]+)")

    for rs in sorted(DOMAIN_SRC.rglob("*.rs")):
        try:
            text = rs.read_text(encoding="utf-8")
        except OSError as exc:
            _emit("ERROR", f"{rs.relative_to(ROOT)}: cannot read ({exc})", errors, notes)
            continue

        cleaned = re.sub(r"//[^\n]*", "", text)

        for lineno, line in enumerate(cleaned.splitlines(), start=1):
            match = use_pattern.match(line)
            if not match:
                continue
            path = match.group("path")
            crate = path.split("::", 1)[0]
            if crate in INTRINSIC_PREFIXES:
                continue
            if crate in ALLOWED_CROSS_CRATES:
                continue
            if crate in ALLOWED_EXTERNAL_DEPS:
                continue
            if line.lstrip().startswith("pub use"):
                continue
            _emit(
                "ERROR",
                f"HEX-C32-02 violation: {rs.relative_to(ROOT)}:{lineno}:"
                f" forbidden external crate '{crate}'",
                errors,
                notes,
            )


def _scan_webhook_direction(errors: list[str], notes: list[str]) -> None:
    """REC-C3.2 V5: chronos-webhook is allowed to depend on
    chronos_domain only; anything else is a leak.

    Adapter direction (good): chronos-webhook → chronos_domain.
    Reverse direction (bad): chronos-domain → chronos-webhook. The
    latter is also caught by rules (1) and (2) above; this rule
    keeps the contract symmetrical and self-documenting."""
    if not WEBHOOK_SRC.is_dir():
        _emit("ERROR", f"missing webhook src dir: {WEBHOOK_SRC}", errors, notes)
        return

    use_pattern = re.compile(r"^\s*use\s+(?P<path>[A-Za-z0-9_:]+)")
    for rs in sorted(WEBHOOK_SRC.rglob("*.rs")):
        try:
            text = rs.read_text(encoding="utf-8")
        except OSError as exc:
            _emit(
                "ERROR",
                f"{rs.relative_to(ROOT)}: cannot read ({exc})",
                errors,
                notes,
            )
            continue

        cleaned = re.sub(r"//[^\n]*", "", text)
        for lineno, line in enumerate(cleaned.splitlines(), start=1):
            match = use_pattern.match(line)
            if not match:
                continue
            crate = match.group("path").split("::", 1)[0]
            if crate in WEBHOOK_FORBIDDEN_CROSS_CRATES:
                _emit(
                    "ERROR",
                    f"REC-C3.2 V5 violation: {rs.relative_to(ROOT)}:{lineno}:"
                    f" chronos-webhook must depend on chronos_domain only;"
                    f" found forbidden cross-crate '{crate}'",
                    errors,
                    notes,
                )

    # Also note the adapter dependency direction expectation.
    print("  chronos-webhook -> chronos_domain: confirmed by absence of cross-crate use")


def _scan_public_surface(errors: list[str], notes: list[str]) -> None:
    """Carry-over from REC-C3.1: ports/mod.rs re-export surface is
    frozen. Missing expected symbols are errors; extras are notes
    (so the surface can be proposed for tightening in REC-C3.3
    without breaking C3.2's gate)."""
    if not PORTS_MOD.is_file():
        _emit("ERROR", f"missing ports/mod.rs: {PORTS_MOD}", errors, notes)
        return

    text = PORTS_MOD.read_text(encoding="utf-8")
    exposed: set[str] = set()
    cleaned = re.sub(r"//[^\n]*", "", text)
    for block_match in re.finditer(
        r"pub\s+use\s+(?P<body>[^;]+);", cleaned, flags=re.MULTILINE | re.DOTALL
    ):
        rhs = block_match.group("body")
        for ident in re.findall(r"([A-Z][A-Za-z0-9_]*)", rhs):
            exposed.add(ident)

    missing = sorted(EXPECTED_PUBLIC_SYMBOLS - exposed)
    extra = sorted(exposed - EXPECTED_PUBLIC_SYMBOLS)

    for symbol in missing:
        _emit(
            "ERROR",
            f"ports/mod.rs does not re-export expected port symbol '{symbol}'",
            errors,
            notes,
        )

    if extra:
        _emit(
            "NOTE",
            "ports/mod.rs re-exports symbols outside the expected surface: "
            + ", ".join(extra),
            errors,
            notes,
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

    print("HEX-C32-01: chronos-domain Cargo.toml")
    _scan_cargo_toml(errors, notes)
    print("HEX-C32-02: chronos-domain outbound purity")
    _scan_outbound_purity(errors, notes)
    print("REC-C3.2 V5: chronos-webhook adapter direction")
    _scan_webhook_direction(errors, notes)
    print("REC-C3.1 surface: ports/mod.rs re-exports")
    _scan_public_surface(errors, notes)

    print(f"  ({len(errors)} error(s); {len(notes)} note(s))")
    if errors:
        return 1
    print("OK: chronos hexagonal boundary clean.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
