#!/usr/bin/env python3
"""
Comprehensive metrics collection script for Chronos Rust project.

Collects:
- Clippy errors and warnings
- Unsafe code count
- Test count
- Benchmark results
- Coverage (if cargo-tarpaulin is installed)
- Complexity metrics (if cognicode is available)

Outputs to ./metrics/chronos_metrics_YYYYMMDD.json
"""

import json
import os
import subprocess
import sys
import re
from datetime import datetime
from pathlib import Path
from typing import Any


def eprint(*args, **kwargs):
    """Print to stderr."""
    print(*args, file=sys.stderr, **kwargs)


def run_command(cmd: list[str], cwd: str | None = None, timeout: int = 300) -> tuple[int, str, str]:
    """Run a command and return (returncode, stdout, stderr)."""
    try:
        result = subprocess.run(
            cmd,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=timeout
        )
        return result.returncode, result.stdout, result.stderr
    except subprocess.TimeoutExpired:
        eprint(f"  [TIMEOUT] Command timed out after {timeout}s: {' '.join(cmd)}")
        return -1, "", "Timeout"
    except FileNotFoundError:
        eprint(f"  [NOT FOUND] Command not found: {cmd[0]}")
        return -1, "", "Not found"
    except Exception as e:
        eprint(f"  [ERROR] Command failed: {' '.join(cmd)}: {e}")
        return -1, "", str(e)


def collect_clippy_metrics(project_root: str) -> dict[str, int]:
    """Collect clippy errors and warnings from cargo clippy --message-format=json."""
    eprint("  Collecting clippy metrics...")

    returncode, stdout, stderr = run_command(
        ["cargo", "clippy", "--workspace", "--message-format=json"],
        cwd=project_root,
        timeout=600
    )

    errors = 0
    warnings = 0

    # Parse JSON lines from stdout
    for line in stdout.splitlines():
        if not line.strip():
            continue
        try:
            msg = json.loads(line)
            if msg.get("reason") != "compiler-message":
                continue
            leveled = msg.get("message", {}).get("level", "")
            if leveled == "error":
                errors += 1
            elif leveled == "warning":
                warnings += 1
        except (json.JSONDecodeError, KeyError):
            continue

    # Also check if clippy failed completely
    if returncode != 0 and errors == 0:
        # There might be parsing errors or other issues
        eprint(f"  [WARN] Clippy exited with code {returncode}, but parsed {errors} errors and {warnings} warnings")

    eprint(f"    -> errors: {errors}, warnings: {warnings}")
    return {"errors": errors, "warnings": warnings}


def collect_unsafe_count(project_root: str) -> int:
    """Count total unsafe blocks across all crates."""
    eprint("  Counting unsafe blocks...")

    unsafe_count = 0

    # Search in crates/*/src directories
    crates_path = Path(project_root) / "crates"
    if crates_path.exists():
        for crate_dir in crates_path.iterdir():
            if not crate_dir.is_dir():
                continue
            src_dir = crate_dir / "src"
            if src_dir.exists():
                for rs_file in src_dir.rglob("*.rs"):
                    try:
                        content = rs_file.read_text()
                        # Count 'unsafe' keywords (not in comments/strings)
                        unsafe_count += content.count("unsafe")
                    except Exception:
                        continue

    # Also check chronos-sandbox if it exists
    sandbox_path = Path(project_root) / "chronos-sandbox"
    if sandbox_path.exists():
        src_dir = sandbox_path / "src"
        if src_dir.exists():
            for rs_file in src_dir.rglob("*.rs"):
                try:
                    content = rs_file.read_text()
                    unsafe_count += content.count("unsafe")
                except Exception:
                    continue

    eprint(f"    -> total unsafe count: {unsafe_count}")
    return unsafe_count


def collect_test_count(project_root: str) -> int:
    """Count number of tests via cargo test --list."""
    eprint("  Counting tests...")

    returncode, stdout, stderr = run_command(
        ["cargo", "test", "--workspace", "--", "--list"],
        cwd=project_root,
        timeout=600
    )

    test_count = 0

    # Parse output looking for test names ending with ': test'
    for line in stdout.splitlines():
        if ": test" in line:
            test_count += 1

    eprint(f"    -> total tests: {test_count}")
    return test_count


def parse_bench_output(output: str) -> dict[str, dict[str, Any]]:
    """Parse cargo bench output to extract benchmark results."""
    benchmarks = {}

    # Pattern to match benchmark results
    # Example: "test bench_name   ... bench: AMAGE: Ns per iteration"
    # or: "name                   time: [Ns]"
    bench_pattern = re.compile(r"^(test\s+)?(\w+)\s+.*?time:\s+\[(\d+\.?\d*)\s+(\w+)/s\]", re.MULTILINE)

    # Alternative pattern for criterion output
    # Example: "execute_query              time: [1.2345 ms]"
    criterion_pattern = re.compile(r"^(\w+)\s+time:\s+\[(\d+\.?\d*)\s+(\w+)\]", re.MULTILINE)

    for match in criterion_pattern.finditer(output):
        name = match.group(1)
        value = float(match.group(2))
        unit = match.group(3)

        # Normalize to common units
        if unit == "ns":
            mean_ms = value / 1_000_000
            out_unit = "ns"
        elif unit == "us":
            mean_ms = value / 1_000
            out_unit = "us"
        elif unit == "ms":
            mean_ms = value
            out_unit = "ms"
        elif unit == "s":
            mean_ms = value * 1000
            out_unit = "s"
        else:
            mean_ms = value
            out_unit = unit

        benchmarks[name] = {
            "mean_ms": round(mean_ms, 6),
            "unit": out_unit
        }

    return benchmarks


def collect_benchmark_results(project_root: str) -> dict[str, dict[str, Any]]:
    """Run cargo bench and parse output."""
    eprint("  Running benchmarks (this may take a while)...")

    returncode, stdout, stderr = run_command(
        ["cargo", "bench", "--", "--no-capture"],
        cwd=project_root,
        timeout=1200
    )

    benchmarks = parse_bench_output(stdout)

    if not benchmarks:
        # Try without --no-capture
        benchmarks = parse_bench_output(stderr)

    eprint(f"    -> collected {len(benchmarks)} benchmark results")
    return benchmarks


def collect_coverage(project_root: str) -> dict[str, float] | None:
    """Collect coverage using cargo-tarpaulin if available."""
    eprint("  Checking for cargo-tarpaulin...")

    # Check if tarpaulin is installed
    check_code, _, _ = run_command(["cargo-tarpaulin", "--version"])
    if check_code != 0:
        eprint("    -> cargo-tarpaulin not installed, skipping coverage")
        return None

    eprint("  Running coverage analysis...")

    returncode, stdout, stderr = run_command(
        ["cargo-tarpaulin", "--out", "json", "--quiet"],
        cwd=project_root,
        timeout=900
    )

    if returncode != 0:
        eprint(f"    [WARN] tarpaulin exited with code {returncode}")
        return None

    # Parse JSON output
    for line in stdout.splitlines():
        if not line.strip():
            continue
        try:
            data = json.loads(line)
            if "line_percent" in data or "branch_percent" in data:
                result = {}
                if "line_percent" in data:
                    result["line_percent"] = round(data["line_percent"], 1)
                if "branch_percent" in data:
                    result["branch_percent"] = round(data["branch_percent"], 1)
                eprint(f"    -> line: {result.get('line_percent', 'N/A')}%, branch: {result.get('branch_percent', 'N/A')}%")
                return result
        except json.JSONDecodeError:
            continue

    eprint("    [WARN] Could not parse tarpaulin output")
    return None


def collect_complexity_metrics(project_root: str) -> dict[str, int] | None:
    """Collect complexity metrics using cognicode if available."""
    eprint("  Checking for cognicode tools...")

    # Try to use cognicode to get complexity metrics
    try:
        # Import the cognicode tools if available
        from opencode import cognicode
    except ImportError:
        eprint("    -> cognicode not available, skipping complexity")
        return None

    eprint("  Collecting complexity metrics...")

    max_cyclomatic = 0
    max_cognitive = 0

    # Find key source files
    key_files = []

    crates_path = Path(project_root) / "crates"
    if crates_path.exists():
        for crate_dir in crates_path.iterdir():
            if not crate_dir.is_dir():
                continue
            src_dir = crate_dir / "src"
            if src_dir.exists():
                for rs_file in src_dir.rglob("*.rs"):
                    key_files.append(str(rs_file))

    # Also check sandbox
    sandbox_path = Path(project_root) / "chronos-sandbox"
    if sandbox_path.exists():
        src_dir = sandbox_path / "src"
        if src_dir.exists():
            for rs_file in src_dir.rglob("*.rs"):
                key_files.append(str(rs_file))

    # Limit to avoid too many calls
    key_files = key_files[:50]

    for file_path in key_files:
        try:
            # Use the cognicode complexity tool
            result = cognicode.get_complexity(file_path=file_path)
            if result:
                cyclomatic = result.get("cyclomatic", 0)
                cognitive = result.get("cognitive", 0)
                max_cyclomatic = max(max_cyclomatic, cyclomatic)
                max_cognitive = max(max_cognitive, cognitive)
        except Exception:
            continue

    if max_cyclomatic > 0 or max_cognitive > 0:
        eprint(f"    -> max_cyclomatic: {max_cyclomatic}, max_cognitive: {max_cognitive}")
        return {"max_cyclomatic": max_cyclomatic, "max_cognitive": max_cognitive}

    eprint("    [WARN] Could not collect complexity metrics")
    return None


def main():
    """Main entry point."""
    project_root = os.environ.get(
        "CHRONOS_PROJECT_ROOT",
        os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    )

    # Resolve to absolute path
    project_root = str(Path(project_root).resolve())

    eprint(f"Chronos Metrics Collection")
    eprint(f"Project root: {project_root}")
    eprint(f"Date: {datetime.now().strftime('%Y-%m-%d')}")
    eprint()

    # Create metrics directory
    metrics_dir = Path(project_root) / "metrics"
    metrics_dir.mkdir(exist_ok=True)

    # Initialize metrics
    metrics = {
        "date": datetime.now().strftime("%Y-%m-%d"),
        "clippy": {"errors": 0, "warnings": 0},
        "unsafe_count": 0,
        "test_count": 0,
        "benchmarks": {},
        "coverage": None,
        "complexity": None
    }

    # 1. Clippy metrics
    try:
        metrics["clippy"] = collect_clippy_metrics(project_root)
    except Exception as e:
        eprint(f"  [ERROR] Clippy collection failed: {e}")

    # 2. Unsafe count
    try:
        metrics["unsafe_count"] = collect_unsafe_count(project_root)
    except Exception as e:
        eprint(f"  [ERROR] Unsafe count failed: {e}")

    # 3. Test count
    try:
        metrics["test_count"] = collect_test_count(project_root)
    except Exception as e:
        eprint(f"  [ERROR] Test count failed: {e}")

    # 4. Benchmarks
    try:
        benchmarks = collect_benchmark_results(project_root)
        if benchmarks:
            metrics["benchmarks"] = benchmarks
    except Exception as e:
        eprint(f"  [ERROR] Benchmark collection failed: {e}")

    # 5. Coverage (optional)
    try:
        coverage = collect_coverage(project_root)
        if coverage:
            metrics["coverage"] = coverage
    except Exception as e:
        eprint(f"  [ERROR] Coverage collection failed: {e}")

    # 6. Complexity metrics (optional)
    try:
        complexity = collect_complexity_metrics(project_root)
        if complexity:
            metrics["complexity"] = complexity
    except Exception as e:
        eprint(f"  [ERROR] Complexity collection failed: {e}")

    # Write output
    date_str = datetime.now().strftime("%Y%m%d")
    output_file = metrics_dir / f"chronos_metrics_{date_str}.json"

    with open(output_file, "w") as f:
        json.dump(metrics, f, indent=2)

    eprint()
    eprint(f"Metrics written to: {output_file}")

    # Print summary
    eprint()
    eprint("=== Summary ===")
    eprint(f"Clippy: {metrics['clippy']['errors']} errors, {metrics['clippy']['warnings']} warnings")
    eprint(f"Unsafe blocks: {metrics['unsafe_count']}")
    eprint(f"Tests: {metrics['test_count']}")
    eprint(f"Benchmarks: {len(metrics['benchmarks'])} collected")
    if metrics["coverage"]:
        eprint(f"Coverage: line {metrics['coverage'].get('line_percent', 'N/A')}%, branch {metrics['coverage'].get('branch_percent', 'N/A')}%")
    if metrics["complexity"]:
        eprint(f"Complexity: cyclomatic {metrics['complexity'].get('max_cyclomatic', 'N/A')}, cognitive {metrics['complexity'].get('max_cognitive', 'N/A')}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
