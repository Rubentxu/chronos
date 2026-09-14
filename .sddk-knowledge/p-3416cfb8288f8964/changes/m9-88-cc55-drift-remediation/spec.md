# Spec — m9-88-cc55-drift-remediation

## Behavioural requirements

### R1. m9-66 apply-checkpoint.json parses without error

- **Given:** `cycle-artifacts/p-3416cfb8288f8964/m9-66-bash-cc-meta-check/apply-checkpoint.json`
  contains an invalid JSON escape sequence on line 35
  (`'^\|'` raw bytes; JSON requires `\\|` for backslash + pipe).
- **When:** A vault CC loads this file via `json.load()`.
- **Then:** `json.load()` succeeds; no `JSONDecodeError` is raised.

### R2. m9-67 base_sha is a valid git commit reachable in repo

- **Given:** `cycle-artifacts/p-3416cfb8288f8964/m9-67-cc-smoke-test/apply-checkpoint.json`
  has `base_sha: "67b3d76bb6e90accefb15cb1f3f55f1b4a52b4dc"`.
- **When:** CC#55 runs `git cat-file -t <base_sha>`.
- **Then:** Command exits 0 and stdout is `commit`.

### R3. m9-85 base_sha is a valid git commit reachable in repo

- **Given:** `cycle-artifacts/p-3416cfb8288f8964/m9-85-cc001-god-module-impl-split/apply-checkpoint.json`
  has `base_sha: "2c2a5cc8f8370eb64dca7fb4ddc47a6f3e8b15a7"`.
- **When:** CC#55 runs `git cat-file -t <base_sha>`.
- **Then:** Command exits 0 and stdout is `commit`.

### R4. Companion files use consistent base_sha

- For each cycle (m9-67, m9-85) where base_sha is fixed:
  - apply-checkpoint.json
  - merge-receipt.md (Base SHA field)
  - release-receipt.md (Base SHA field)
  - verify-report.md (Base column or context)
  - verify-findings.json (base_sha field, if present)
- **Then:** All 5 files reference the same base_sha value, matching
  the actual git parent of the cycle's head_sha.

### R5. m9-79 peel_match is True (bonus)

- **Given:** m9-79 head_sha == remote_tag_peel (both `f41abd45...`).
- **When:** CC#3 reads `apply-checkpoint.json["peel_match"]`.
- **Then:** Value is `true`.

## Non-functional requirements

- All fixed files remain valid JSON.
- All JSON re-serializations preserve semantic content (only formatting
  changes — key order, array indentation — are acceptable).
- The fix is atomic: one vault commit fixes all 4 drift points.
- `bash scripts/check_vault_drift.sh` after the fix reports CC#55 with
  0 drift lines. Other CCs may report drift (pre-existing, out of
  scope).

## Acceptance scenarios

### S1. CC#55 reports 0 drift after fix

```python
# After m9-88 vault commit:
result = subprocess.run(['bash', 'scripts/check_vault_drift.sh'], capture_output=True)
# Assert: CC#55 not in the DRIFT summary
```

### S2. Each fixed base_sha is reachable in git

```python
import subprocess
for sha in [
    '67b3d76a80ec8e766a0689fba810fc499b5cd4a4',  # m9-67 base
    '72e120c2e2bb9774c459ef516dcf3e7b21ef90c3',  # m9-85 base
]:
    result = subprocess.run(['git', 'cat-file', '-t', sha], capture_output=True)
    assert result.stdout.strip() == b'commit'
```

### S3. m9-66 JSON parses cleanly

```python
import json
content = open('cycle-artifacts/p-3416cfb8288f8964/m9-66-bash-cc-meta-check/apply-checkpoint.json').read()
json.loads(content)  # must not raise JSONDecodeError
```
