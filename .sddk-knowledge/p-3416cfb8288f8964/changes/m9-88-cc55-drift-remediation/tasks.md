# Tasks — m9-88-cc55-drift-remediation

## Slice 1 — vault CC#55 drift fixes (4 atomic edits)

### Task 1.1 — fix m9-66 JSON escape (line 35)

- **File:** `cycle-artifacts/p-3416cfb8288f8964/m9-66-bash-cc-meta-check/apply-checkpoint.json`
- **Edit:** Replace `'^\| (m6'` with `'^\\\\| (m6'` (which is
  `'^\\|'` on the wire, i.e. `'^\|'` as a Python string literal —
  which is the regex characters `^\|` correctly JSON-encoded).
- **Verify:** `python3 -c "import json; json.load(open('...'))"`
  succeeds.
- **Status:** DONE.

### Task 1.2 — fix m9-67 base_sha (5 files)

- **Target base_sha:** `67b3d76a80ec8e766a0689fba810fc499b5cd4a4`
  (real parent of head_sha `5c83df9ce96862c0c95598d63cddb147e3ea6ab5`
  on the source commit; vault commit head_sha `ef21e1fe` has parent
  `5c83df9`).
- **Files:**
  - apply-checkpoint.json (head=ef21e1fe, base=5c83df9)
  - merge-receipt.md (Base SHA field)
  - release-receipt.md (Base SHA field)
  - verify-findings.json (head=5c83df9, base=67b3d76a80ec)
  - verify-report.md (Base column)
- **Verify:** `git cat-file -t` returns `commit` for each base_sha.
- **Status:** DONE.

### Task 1.3 — fix m9-85 base_sha (4 files)

- **Target base_sha:** `72e120c2e2bb9774c459ef516dcf3e7b21ef90c3`
  (real parent of head_sha `a85034603031f2dd1dc340d78d84f71f140672e0`).
- **Files:**
  - apply-checkpoint.json
  - merge-receipt.md (Base SHA field)
  - release-receipt.md (Base SHA field)
  - verify-report.md (Base SHA line + cross-check line)
- **Verify:** `git cat-file -t` returns `commit` for `72e120c2e2bb9774c459ef516dcf3e7b21ef90c3`.
- **Status:** DONE.

### Task 1.4 — set m9-79 peel_match (bonus)

- **File:** `cycle-artifacts/p-3416cfb8288f8964/m9-79-attach-capability-type/apply-checkpoint.json`
- **Edit:** `peel_match: None` → `peel_match: true`.
- **Rationale:** head_sha `f41abd45...` equals remote_tag_peel
  `f41abd45...` (verified in git). The peel_match should be True.
- **Status:** DONE.

## Slice 2 — commit + merge + tag + cascade SHAs

### Task 2.1 — commit on feat/m9-88-cc55-drift-remediation

- Already done at `78ec386` (11 files, 46 insertions, 24 deletions).

### Task 2.2 — `--no-ff` merge into main

### Task 2.3 — tag v0.7.90 at merge commit (CC#42 fixpoint-cascade)

### Task 2.4 — push to origin

### Task 2.5 — `python3 scripts/regen_manifest_index_shas.py` fixpoint

## Slice 3 — cycle artifacts + handoff

### Task 3.1 — implementation-receipt.md
### Task 3.2 — verify-report.md + verify-findings.json
### Task 3.3 — release-report.md + release-receipt.md
### Task 3.4 — merge-receipt.md
### Task 3.5 — apply-checkpoint.json
### Task 3.6 — archive-manifest.md with Evidence bindings + Cross-checks
### Task 3.7 — cycles/index.md Total cycles 88 + handoff

## Estimated work

- Tasks 1.1-1.4: DONE.
- Tasks 2.1-2.5: ~10 min.
- Tasks 3.1-3.7: ~15 min.

Total: ~25 min wall time remaining.

## Risk mitigation

- Round-trip: each fixed base_sha verified in git before commit.
- Public surface: no source code changed; only vault artifacts
  modified. chronos binary behavior unchanged.
- CC#55 verified clean post-fix.
