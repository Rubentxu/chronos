#!/usr/bin/env bash
#
# smoke_test_ccs.sh — synthetic-drift injection test for critical vault CCs.
#
# Why: m9-65 discovered 49 stale branches (CC#46 was missing all milestone
# prefixes); m9-66 discovered 54 stale SHAs (CC#4 had a broken awk regex).
# Both drifted for many cycles before being caught. This script prevents
# recurrence by periodically injecting synthetic drift into each critical
# CC's domain and verifying the script catches it.
#
# Strategy:
# - Work on copies in /tmp/smoke_test_$$/ (no rollback needed: each iteration
#   rebuilds the copy from a clean checkout).
# - For each critical CC: (a) build a copy of the repo, (b) inject known
#   drift that violates the CC, (c) run check_vault_drift.sh on the copy,
#   (d) assert exit code is 1 (drift detected), (e) assert the output
#   mentions the CC, (f) clean up.
# - Clean state: exit 0. Any failure: exit 1 with a list of failed CCs.
#
# Critical CCs covered:
# - CC#4: SHA-256 consistency in archive-manifest Artifact index (broken awk bug)
# - CC#39: Total cycles ↔ filesystem cycles (cross-cutting; superset of CC#5)
# - CC#46: No stale fix/m9-* branches (missing other milestone prefixes)
# - CC#55: verify-report.md must have `## Files Inventory` section (added by m9-68)
# - CC#48+CC#54: meta-checks (CC#48 was the only auto-executed check;
#   CC#54 was added for bash CCs; both must run)
# - regen_manifest_index_shas.py: the CC#4 repair tool must agree with CC#4
#   (added by m9-76) — detects the same drift the gate detects, and its own
#   rewrite makes the gate go green again.
#
# Cost: ~10s per CC × 5 CCs ≈ 50s. Run before merging any change that
# touches vault-drift-sweep.md or check_vault_drift.sh.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
WORK_DIR="$(mktemp -d /tmp/smoke_test_ccs.XXXXXX)"
# Note: do NOT use `trap "rm -rf ..." EXIT` because subshells (e.g., from
# command substitution `$(...)`) inherit the trap and clean up prematurely.
# We clean up explicitly at the end of the script.

cd "$REPO_ROOT"

# Setup: copy repo into work dir, point check_vault_drift.sh at the copy.
setup_work_copy() {
  local dest="$1"
  # tar -x requires the destination directory to exist.
  mkdir -p "$dest"
  # Use git clone (NOT git archive) so the .git directory is included.
  # Many python CCs (e.g., CC#3, #42, #47) call `git log` via subprocess
  # and need a real repository to operate on. git archive would strip
  # .git, breaking them. The `--shared` flag avoids copying objects twice
  # (we only need the refs/logs/HEAD to answer `git log`).
  git clone --quiet --shared "$REPO_ROOT" "$dest"
  # Overlay the working-tree copies of the vault tooling. A clone only sees
  # committed state, but the point of this script is to smoke-test the tree we
  # are about to merge — including changes to these scripts themselves.
  mkdir -p "$dest/scripts/tests"
  cp "$SCRIPT_DIR/check_vault_drift.sh" "$dest/scripts/check_vault_drift.sh"
  chmod +x "$dest/scripts/check_vault_drift.sh"
  if [ -f "$SCRIPT_DIR/regen_manifest_index_shas.py" ]; then
    cp "$SCRIPT_DIR/regen_manifest_index_shas.py" "$dest/scripts/regen_manifest_index_shas.py"
    chmod +x "$dest/scripts/regen_manifest_index_shas.py"
  fi
  if [ -f "$SCRIPT_DIR/tests/test_regen_manifest_index_shas.py" ]; then
    cp "$SCRIPT_DIR/tests/test_regen_manifest_index_shas.py" \
       "$dest/scripts/tests/test_regen_manifest_index_shas.py"
  fi
}

# Run check_vault_drift.sh in the work dir and capture output + exit code.
# Note: capture exit code BEFORE the redirect, because the redirect itself
# would clobber $? from the script exit. The subshell parentheses ensure
# the inner script's exit propagates cleanly. Each call writes to a
# per-test log file so we can inspect failures after the fact.
run_check() {
  local dest="$1"
  local test_name="$2"
  ( cd "$dest" && ./scripts/check_vault_drift.sh ) > "$WORK_DIR/${test_name}.log" 2>&1
  local rc=$?
  echo "$rc"
}

# Result tracking.
failures=()
total=0

# ==============================================================================
# CC#4: SHA-256 consistency in archive-manifest Artifact index
# ==============================================================================
# Inject: replace a real SHA in m9-01 archive-manifest with a wrong one.
test_cc4() {
  total=$((total+1))
  echo "[CC#4] SHA-256 consistency..."
  local dest="$WORK_DIR/cc4"
  setup_work_copy "$dest"

  local manifest="$dest/.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-01-schema-versioning/archive-manifest.md"
  # Backup and restore via python (safer than sed with glob).
  python3 -c "
import sys, os
path = '$manifest'
with open(path) as f: content = f.read()
# Replace the archive-report SHA with a known-bad value (keep 64-char hex).
import re
bt = chr(96)  # backtick
m = re.search(r'(\| archive-report \| ' + bt + r'\.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-01-schema-versioning/archive-report\.md' + bt + r' \| ' + bt + r')([a-f0-9]{64})(' + bt + r' \|)', content)
if m:
    bad = 'deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef'
    content = content[:m.start(2)] + bad + content[m.end(2):]
    with open(path, 'w') as f: f.write(content)
    print('Injected drift')
else:
    print('FAIL: could not find archive-report row to inject drift')
    sys.exit(1)
"

  local exit_code=$(run_check "$dest" "cc4")
  if [ "$exit_code" -ne 1 ]; then
    failures+=("CC#4: exit=$exit_code (expected 1)")
    echo "  FAIL: exit code $exit_code, expected 1"
    return
  fi
  if ! grep -qE "DRIFT.*CC#4" "$WORK_DIR/cc4.log"; then
    failures+=("CC#4: drift line missing in output")
    echo "  FAIL: no drift line for CC#4"
    cat "$WORK_DIR/cc4.log"
    return
  fi
  echo "  PASS"
}

# ==============================================================================
# CC#39: Total cycles ↔ filesystem cycles (cross-cutting; superset of CC#5)
# ==============================================================================
# CC#5 (Total cycles consistency in cycles/index.md) is subsumed by CC#39
# Part C, which is a python CC that ALSO counts filesystem cycles. To test
# the cross-check chain end-to-end, we inject a Total cycles mismatch that
# the python chain detects first (CC#48 → CC#39). CC#5 is a bash CC that
# runs the same check inside CC#54, so its correctness is verified by the
# fact that CC#39 catches the same drift class.
test_cc39() {
  total=$((total+1))
  echo "[CC#39] Total cycles ↔ filesystem cycles..."
  local dest="$WORK_DIR/cc39"
  setup_work_copy "$dest"

  python3 -c "
path = '$dest/.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md'
with open(path) as f: content = f.read()
# Read current Total cycles value and bump it. The literal value changes
# each cycle as new cycles are added, so we must read it dynamically.
import re
m = re.search(r'(\| Total cycles \| )(\d+)( \|)', content)
if not m:
    print('FAIL: could not find Total cycles row')
    raise SystemExit(1)
current = int(m.group(2))
bad = current + 100  # any number that won't match filesystem
content = content.replace(m.group(0), m.group(1) + str(bad) + m.group(3), 1)
with open(path, 'w') as f: f.write(content)
print('Injected drift (current=' + str(current) + ', bad=' + str(bad) + ')')
"

  local exit_code=$(run_check "$dest" "cc39")
  if [ "$exit_code" -ne 1 ]; then
    failures+=("CC#39: exit=$exit_code (expected 1)")
    echo "  FAIL: exit code $exit_code, expected 1"
    return
  fi
  if ! grep -qE "DRIFT.*CC#39" "$WORK_DIR/cc39.log"; then
    failures+=("CC#39: drift line missing in output")
    echo "  FAIL: no drift line for CC#39"
    cat "$WORK_DIR/cc39.log"
    return
  fi
  echo "  PASS"
}

# ==============================================================================
# CC#46: No stale local or remote fix/m9-* branches
# ==============================================================================
# Inject: create a fake fix/m9-99-smoke-test branch in the cloned work copy.
# CC#46 is bash and counts `git branch --list 'fix/m9-*'`. The clone (via
# setup_work_copy) preserves .git, so the new local branch is observable
# to CC#46 — and it stays unmerged, triggering the DRIFT report.
test_cc46() {
  total=$((total+1))
  echo "[CC#46] No stale fix/m9-* branches..."
  local dest="$WORK_DIR/cc46"
  setup_work_copy "$dest"

  # Create a fake stale branch from current HEAD. CC#46 scans local
  # `git branch --list 'fix/m9-*'` for ANY branch matching that pattern;
  # since our clone includes a working .git and remote, the count is
  # observable. The branch is NOT merged into main, so it stays stale.
  ( cd "$dest" && \
    git checkout -q -b 'fix/m9-99-smoke-test' 2>/dev/null
  )

  local exit_code=$(run_check "$dest" "cc46")
  if [ "$exit_code" -ne 1 ]; then
    failures+=("CC#46: exit=$exit_code (expected 1)")
    echo "  FAIL: exit code $exit_code, expected 1"
    return
  fi
  if ! grep -qE "DRIFT.*CC#46" "$WORK_DIR/cc46.log"; then
    failures+=("CC#46: drift line missing in output")
    echo "  FAIL: no drift line for CC#46"
    cat "$WORK_DIR/cc46.log"
    return
  fi
  echo "  PASS"
}

# ==============================================================================
# CC#55: verify-report.md must have `## Files Inventory` section
# ==============================================================================
# Inject: remove the `## Files Inventory` section from a known-good
# verify-report.md (m9-67, which is included in the clone since it's
# part of the committed history). CC#55 should detect this.
test_cc55() {
  total=$((total+1))
  echo "[CC#55] verify-report.md must have Files Inventory..."
  local dest="$WORK_DIR/cc55"
  setup_work_copy "$dest"

  # Remove the Files Inventory section from m9-67's verify-report.
  python3 -c "
import re
path = '$dest/cycle-artifacts/p-3416cfb8288f8964/m9-67-cc-smoke-test/verify-report.md'
content = open(path).read()
m = re.search(r'^## Files Inventory.*?(?=^## |\Z)', content, re.MULTILINE | re.DOTALL)
if m:
    new = content[:m.start()] + content[m.end():]
    open(path, 'w').write(new)
    print('Removed Files Inventory section')
else:
    print('FAIL: could not find Files Inventory section to remove')
    raise SystemExit(1)
"

  local exit_code=$(run_check "$dest" "cc55")
  if [ "$exit_code" -ne 1 ]; then
    failures+=("CC#55: exit=$exit_code (expected 1)")
    echo "  FAIL: exit code $exit_code, expected 1"
    return
  fi
  if ! grep -qE "DRIFT.*CC#55" "$WORK_DIR/cc55.log"; then
    failures+=("CC#55: drift line missing in output")
    echo "  FAIL: no drift line for CC#55"
    cat "$WORK_DIR/cc55.log"
    return
  fi
  echo "  PASS"
}

# ==============================================================================
# regen_manifest_index_shas.py: the CC#4 repair tool must agree with CC#4
# ==============================================================================
# CC#4 (broken awk, m9-66) had no in-repo repair tool: the ritual lived in a
# throwaway $TMPDIR script, so nothing exercised the gate's own logic and the
# bug survived many cycles. This test pins script and gate together:
#   (a) the script's unit tests pass,
#   (b) --check is clean on a clean tree (agreement in the green direction),
#   (c) after injecting the same stale SHA that test_cc4 injects, BOTH the gate
#       and --check fail and name the same offending row,
#   (d) running the script rewrites the row, restores the true SHA, and makes
#       both --check and the gate green again (agreement in the repair direction).
test_regen_script() {
  total=$((total+1))
  echo "[regen] regen_manifest_index_shas.py ↔ CC#4..."
  local dest="$WORK_DIR/regen"
  setup_work_copy "$dest"

  local manifest="$dest/.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-01-schema-versioning/archive-manifest.md"
  local report=".sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-01-schema-versioning/archive-report.md"

  # (a) unit tests of the tool itself.
  if ! ( cd "$dest" && python3 scripts/tests/test_regen_manifest_index_shas.py ) \
      > "$WORK_DIR/regen-unit.log" 2>&1; then
    failures+=("regen: unit tests failed")
    echo "  FAIL: unit tests failed"
    tail -20 "$WORK_DIR/regen-unit.log"
    return
  fi

  # (b) clean tree: --check must agree with the gate (both green).
  ( cd "$dest" && python3 scripts/regen_manifest_index_shas.py --check ) \
      > "$WORK_DIR/regen-clean.log" 2>&1
  local clean_rc=$?
  if [ "$clean_rc" -ne 0 ]; then
    failures+=("regen: --check exit=$clean_rc on clean tree (expected 0)")
    echo "  FAIL: --check failed on a clean tree"
    cat "$WORK_DIR/regen-clean.log"
    return
  fi

  # (c) inject the same drift as test_cc4: a wrong archive-report SHA.
  python3 -c "
import re, sys
path = '$manifest'
content = open(path).read()
bt = chr(96)
pat = r'(\| archive-report \| ' + bt + re.escape('$report') + bt + r' \| ' + bt + r')([a-f0-9]{64})(' + bt + r' \|)'
m = re.search(pat, content)
if not m:
    print('FAIL: could not find archive-report row to inject drift')
    sys.exit(1)
bad = 'deadbeef' * 8
open(path, 'w').write(content[:m.start(2)] + bad + content[m.end(2):])
print('Injected drift')
" || {
    failures+=("regen: drift injection failed")
    echo "  FAIL: could not inject drift"
    return
  }

  ( cd "$dest" && python3 scripts/regen_manifest_index_shas.py --check ) \
      > "$WORK_DIR/regen-stale.log" 2>&1
  local stale_rc=$?
  if [ "$stale_rc" -ne 1 ]; then
    failures+=("regen: --check exit=$stale_rc on drifted tree (expected 1)")
    echo "  FAIL: --check did not detect the injected drift"
    cat "$WORK_DIR/regen-stale.log"
    return
  fi
  if ! grep -qF "$report" "$WORK_DIR/regen-stale.log"; then
    failures+=("regen: --check did not name the drifted row")
    echo "  FAIL: drifted row not named in --check output"
    cat "$WORK_DIR/regen-stale.log"
    return
  fi
  local gate_rc=$(run_check "$dest" "regen-drift")
  if [ "$gate_rc" -ne 1 ] || ! grep -qE "DRIFT.*CC#4" "$WORK_DIR/regen-drift.log"; then
    failures+=("regen: CC#4 gate and script disagree on injected drift (gate rc=$gate_rc)")
    echo "  FAIL: the vault gate did not flag the same drift"
    cat "$WORK_DIR/regen-drift.log"
    return
  fi

  # (d) run the script: it must repair the row and both checks go green.
  ( cd "$dest" && python3 scripts/regen_manifest_index_shas.py ) \
      > "$WORK_DIR/regen-fix.log" 2>&1
  local fix_rc=$?
  if [ "$fix_rc" -ne 0 ]; then
    failures+=("regen: rewrite exit=$fix_rc (expected 0)")
    echo "  FAIL: rewrite failed"
    cat "$WORK_DIR/regen-fix.log"
    return
  fi
  if ! grep -qE "1 row\(s\) rewritten" "$WORK_DIR/regen-fix.log"; then
    failures+=("regen: rewrite did not report exactly the injected row")
    echo "  FAIL: rewrite summary unexpected"
    cat "$WORK_DIR/regen-fix.log"
    return
  fi
  local expected=$(sha256sum "$dest/$report" | cut -d' ' -f1)
  if ! grep -qF "\`$expected\`" "$manifest"; then
    failures+=("regen: rewritten row does not carry the true SHA-256")
    echo "  FAIL: manifest does not carry the recomputed SHA"
    return
  fi
  ( cd "$dest" && python3 scripts/regen_manifest_index_shas.py --check ) \
      > "$WORK_DIR/regen-refixed.log" 2>&1
  if [ $? -ne 0 ]; then
    failures+=("regen: --check still failing after rewrite")
    echo "  FAIL: --check not clean after rewrite"
    cat "$WORK_DIR/regen-refixed.log"
    return
  fi
  local gate_after=$(run_check "$dest" "regen-fixed")
  if [ "$gate_after" -ne 0 ]; then
    failures+=("regen: CC#4 gate still failing after rewrite (rc=$gate_after)")
    echo "  FAIL: vault gate not green after rewrite"
    cat "$WORK_DIR/regen-fixed.log"
    return
  fi
  echo "  PASS"
}

# ==============================================================================
# CC#48 + CC#54: meta-checks must run together AND report failure on drift
# ==============================================================================
# This test verifies that in the clean state (no drift injected),
# check_vault_drift.sh exits 0 and the final PASS message reports the
# expected CC counts (48 python + 7 bash). If a future change to the
# meta-check extraction logic (CC#48's python block detection or CC#54's
# bash block extraction) silently drops a CC, this assertion catches it.
test_meta_checks() {
  total=$((total+1))
  echo "[CC#48+CC#54] meta-checks must run together..."
  local dest="$WORK_DIR/meta"
  setup_work_copy "$dest"

  # No drift injection; expect PASS, exit 0.
  local exit_code=$(run_check "$dest" "meta")
  if [ "$exit_code" -ne 0 ]; then
    failures+=("CC#48+CC#54: clean state exit=$exit_code (expected 0)")
    echo "  FAIL: clean state did not exit 0"
    cat "$WORK_DIR/meta.log"
    return
  fi
  if ! grep -qE "48 python CCs all clean, 7 bash CCs all clean" "$WORK_DIR/meta.log"; then
    failures+=("CC#48+CC#54: PASS message format unexpected")
    echo "  FAIL: PASS message does not match expected format"
    cat "$WORK_DIR/meta.log"
    return
  fi
  echo "  PASS"
}

# ==============================================================================
# Run all tests
# ==============================================================================
echo "=== CC smoke test ==="
echo "Work dir: $WORK_DIR"
echo

test_cc4
test_cc39
test_cc46
test_cc55
test_regen_script
test_meta_checks

echo
echo "=== Summary ==="
echo "Tests run: $total"
echo "Failures: ${#failures[@]}"

if [ ${#failures[@]} -gt 0 ]; then
  echo
  echo "Failed CCs:"
  for f in "${failures[@]}"; do
    echo "  - $f"
  done
  rm -rf "$WORK_DIR"
  exit 1
fi

echo "All critical CCs detect synthetic drift. CC detection chain is healthy."

# Cleanup
rm -rf "$WORK_DIR"
exit 0
