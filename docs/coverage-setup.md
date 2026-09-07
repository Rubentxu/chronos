# Coverage Setup Guide

This document describes how to set up and run code coverage tracking for the Chronos project using cargo-tarpaulin.

## Overview

Chronos uses [cargo-tarpaulin](https://github.com/xd009642/tarpaulin) for code coverage analysis. Coverage reports help identify untested code paths and measure test effectiveness over time.

## Local Setup

### Prerequisites

- Rust toolchain (1.75+)
- `cargo-tarpaulin` installed

### Installing cargo-tarpaulin

```bash
cargo install cargo-tarpaulin
```

Note: cargo-tarpaulin is installed as a binary tool, not a project dependency, because:
1. It's a development utility, not a library
2. It requires building with the same compiler version as the project
3. It has significant build time and shouldn't be part of regular builds

### Running Coverage

From the project root:

```bash
./scripts/run_coverage.sh
```

This will:
1. Check if cargo-tarpaulin is installed (and install if needed)
2. Run tarpaulin on the entire workspace
3. Generate reports in multiple formats (HTML, JSON, XML)
4. Upload to codecov.io if `CODECOV_TOKEN` is set

Reports are generated in `./metrics/coverage/`:
- `tarpaulin-report.html` - Human-readable HTML report
- `tarpaulin.json` - JSON format for tooling
- `tarpaulin.xml` - Cobertura XML format (used by codecov)

## Understanding Coverage Percentages

cargo-tarpaulin reports line coverage, which indicates:

| Coverage | Interpretation |
|----------|-----------------|
| 90-100%  | Excellent - most code is tested |
| 70-89%   | Good - most critical paths covered |
| 50-69%   | Moderate - gaps exist |
| <50%     | Low - significant testing effort needed |

**Note**: 100% coverage does not guarantee bug-free code. It only confirms that every line was executed at least once. Edge cases, error handling, and integration scenarios still need explicit testing.

## Interpreting the HTML Report

Open `tarpaulin-report.html` in a browser:

1. **File Tree** (left panel) - Navigate between crates and source files
2. **Coverage Colors**:
   - 🟢 Green - Lines covered by tests
   - 🔴 Red - Lines not covered
   - 🟡 Yellow - Partial branch coverage
3. **Metrics per file**:
   - Lines hit / total lines
   - Functions covered / total functions
   - Branch coverage percentage

### Key Areas to Check

1. **Error handling paths** - Ensure `unwrap()`, `expect()`, and error branches are tested
2. **Edge cases** - Empty inputs, boundary values, concurrent access
3. **Critical business logic** - Core domain functions should have high coverage

## CI/CD Integration

### GitHub Actions

The coverage workflow runs on:
- Push to `main` or `develop` branches
- Pull requests to `main`

### Setting Up Codecov

1. Sign up at [codecov.io](https://codecov.io)
2. Add your repository to codecov
3. Copy the repository upload token
4. Add as GitHub secret: `CODECOV_TOKEN`

To add the secret:
1. Go to repository Settings → Secrets and variables → Actions
2. Click "New repository secret"
3. Name: `CODECOV_TOKEN`
4. Value: paste the token from codecov.io

### Viewing Coverage History

After first successful run:
1. Go to codecov.io and select your repository
2. View coverage trends over time
3. Set up coverage targets and alerts
4. Compare coverage between branches

## Excluding Code from Coverage

To exclude files or functions from coverage reports, use cargo-tarpaulin's ignore features:

```rust
#[cfg(not(tarpaulin))]
fn expensive_debug_only_function() { ... }
```

Or add to `pyproject.toml` or a dedicated `tarpaulin.toml`:

```toml
[tarpaulin]
ignore-files = ["generated/*.rs", "tests/**/*.rs"]
```

## Troubleshooting

### Long build times
cargo-tarpaulin builds the project with instrumentation. Use incremental builds when possible.

### Out of memory
For large projects, run coverage per-crate:
```bash
cargo tarpaulin -p chronos-domain --out html -o ./metrics/coverage/
```

### Missing coverage for macro expansions
Some macro-generated code may not appear in coverage reports. This is a known limitation of instrumentation-based coverage tools.
