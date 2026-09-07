#!/bin/bash
#
# Save baseline metrics for Chronos project
#
# This script:
# 1. Runs the metrics collection
# 2. Copies the result to ./metrics/baseline.json
# 3. Prints a summary of the baseline values
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
METRICS_DIR="$PROJECT_ROOT/metrics"

echo "=== Chronos Baseline Metrics Collection ==="
echo "Project root: $PROJECT_ROOT"
echo ""

# Run the metrics collection script
echo "Running metrics collection..."
python3 "$SCRIPT_DIR/collect_metrics.py"

# Find the latest metrics file
LATEST_FILE=$(ls -t "$METRICS_DIR"/chronos_metrics_*.json 2>/dev/null | head -1)

if [ -z "$LATEST_FILE" ]; then
    echo "ERROR: No metrics file found after collection"
    exit 1
fi

echo ""
echo "Latest metrics file: $LATEST_FILE"

# Copy to baseline
cp "$LATEST_FILE" "$METRICS_DIR/baseline.json"
echo "Copied to: $METRICS_DIR/baseline.json"

echo ""
echo "=== Baseline Summary ==="

# Extract and print key metrics using python
python3 -c "
import json
import sys

with open('$METRICS_DIR/baseline.json', 'r') as f:
    metrics = json.load(f)

print(f\"Date: {metrics.get('date', 'N/A')}\")
print()
print('Clippy:')
print(f\"  Errors:   {metrics.get('clippy', {}).get('errors', 0)}\")
print(f\"  Warnings: {metrics.get('clippy', {}).get('warnings', 0)}\")
print()
print(f\"Unsafe blocks: {metrics.get('unsafe_count', 0)}\")
print(f\"Tests:         {metrics.get('test_count', 0)}\")
print()

benchmarks = metrics.get('benchmarks', {})
if benchmarks:
    print('Benchmarks:')
    for name, data in sorted(benchmarks.items()):
        mean = data.get('mean_ms', 0)
        unit = data.get('unit', 'ms')
        print(f\"  {name}: {mean} {unit}\")
    print()

coverage = metrics.get('coverage')
if coverage:
    print('Coverage:')
    print(f\"  Line:   {coverage.get('line_percent', 'N/A')}%\")
    print(f\"  Branch: {coverage.get('branch_percent', 'N/A')}%\")
    print()

complexity = metrics.get('complexity')
if complexity:
    print('Complexity:')
    print(f\"  Max Cyclomatic: {complexity.get('max_cyclomatic', 'N/A')}\")
    print(f\"  Max Cognitive:  {complexity.get('max_cognitive', 'N/A')}\")
"

echo ""
echo "Baseline saved successfully!"
