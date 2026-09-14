# Tasks: m9-83 CC#39 Total cycles off-by-one

> Tasks are reviewable work units.

## T0 — Vault files on cycle branch

**Files:**
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-83-cc39-total-cycles/exploration-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-83-cc39-total-cycles/proposal.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-83-cc39-total-cycles/spec.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-83-cc39-total-cycles/tasks.md`
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-83 OPEN row)
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last updated bump)

**Commit message:**
> m9-83: vault (exploration-report + proposal + spec + tasks)

## T1 — Apply the literal fix

**File:** `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`
change `| Total cycles | 84 |` → `| Total cycles | 82 |`.

**Acceptance:**
- `bash scripts/check_vault_drift.sh` exits 0.
- `python3 scripts/regen_manifest_index_shas.py --check` exits 0.

**Commit message:**
> m9-83: correct cycles/index.md Total cycles (84 → 82) to match row count

## T2 — Vault index updates

**Files:** `cycles/index.md` (m9-83 CLOSED row), `terms/index.md` (CC#39 closure recorded).

**Commit message:**
> m9-83: vault (cycles + terms)

## T3 — Verify-phase cycle-artifacts

**Files:** `cycle-artifacts/p-3416cfb8288f8964/m9-83-cc39-total-cycles/`
containing `apply-checkpoint.json`, `implementation-receipt.md`,
`verify-findings.json`, `verify-report.md`.

**Commit message:**
> m9-83: verify-phase cycle-artifacts

## T4 — Release phase (merge + tag)

Cycle branch merged --no-ff into main → merge SHA recorded. Tag
`v0.7.85` on the merge commit. Receipts: `release-receipt.md`,
`merge-receipt.md`, `release-report.md`.

**Commit message:**
> m9-83: release-phase artifacts (v0.7.85)

## T5 — Archive phase

`archive-manifest.md` + `change-entry.md`. Cycle branch deleted.

**Commit message:**
> m9-83: archive phase

## T6 — Apply-checkpoint status flip

**File:** `cycle-artifacts/.../apply-checkpoint.json` — `status: archived`.

**Commit message:**
> m9-83: apply-checkpoint status=archived

## T7 — Handoff persistence

**File:** `.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-backlog-blocked-2026-09-12.md`
— append new session section.

**Commit message:**
> docs(handoff): append m9-83 closure
