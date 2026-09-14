# Proposal — m9-89 cascade CC cleanup across m9-77..m9-88

## Intent

Close the cascading CC drift surfaced by m9-88's m9-66 JSON abort fix.

## Scope

- 12 CCs (3, 7, 8, 11, 12, 14, 15, 22, 23, 29, 40, 43) across 11 cycles (m9-77..m9-87) + 1 cycle (m9-88) + 1 change-entry (m9-67).
- 1 supporting CC (CC#4) for archive-manifest SHA-256 regen.
- Vault-only — no Rust source code.

## Approach

Two reusable Python tools in `scripts/`:

1. `scripts/audit_m9_89_cascade.py` — enumerates the per-cycle, per-CC fix catalog (JSON). Documented and reusable.
2. `scripts/fix_m9_89_cascade.py` — applies mechanical backfills. Idempotent; `--dry-run` mode.

Plus the existing `scripts/regen_manifest_index_shas.py` for the CC#4 SHA-256 cascade.

## Out-of-scope (deferred)

- CC#46/CC#53 stale branches (12 merged-into-main feat/* branches from m9-67..m9-78) — FIND-M9-71 hardening cycle.
- FIND-M9-81 (sddk CLI bug) — external-deferred from m9-88.

## Tiers

| Tier | Status | Notes |
|---|---|---|
| T0 (fmt + clippy) | required | ensures no Rust regression |
| T1 (lib unit) | required | ensures no Rust regression |
| T2 (per-crate integration) | not required | vault-only cycle |
| T4 (sandbox smoke) | not required | vault-only cycle |

## Deliverables

1. **Vault fix tool**: `scripts/fix_m9_89_cascade.py` + `scripts/audit_m9_89_cascade.py`.
2. **Backfilled artifacts**: 111 files across `cycle-artifacts/` and `.sddk-knowledge/`.
3. **Cycle artifacts** (m9-89): apply-checkpoint.json, implementation-receipt.md, verify-findings.json, verify-report.md, merge-receipt.md, release-receipt.md, release-report.md, archive-manifest.md, change-entry.md.
4. **Cycles index** update: m9-89 row + Total cycles = 89.
5. **Handoff** to next cycle: closure note in `.sddk-knowledge/p-3416cfb8288f8964/handoff/`.

## Risks

- The fix tool makes 197 changes. Some are subtle:
  - `head_sha` realignment for 6 cycles (m9-77, m9-80, m9-81, m9-82, m9-87, m9-88) — the cycle's "real" head was the refactor commit; the aligned head is the merge commit where the tag lives. This is the canonical CC#3 fix.
  - archive-manifest Head SHA backticks (m9-83..m9-87) — adds backticks to match the CC#8 regex `\| Head SHA \| \`([a-f0-9]+)\``.
  - merge-receipt Head SHA / Base SHA / Branch / Date added in table format for 52 cycles (m9-34..m9-86) — CC#23 regex requires table format. Some cycles had key-value format that pre-dates the canonical.
- The cascade fix tool's `head_sha_align_peel` step is irreversible — once `head_sha` is moved from X (refactor) to Z (merge), the cycle's "real" code change is no longer pointed to by `head_sha`. The merge commit (Z) contains the same code, but `head_sha` no longer points to the commit that introduced it.

  This is acceptable per CC#3's intent (head_sha should be the cycle's
  published head, where the tag lives), but it changes the meaning of
  `head_sha` for these cycles. Documented in `apply-checkpoint.json`
  `notes.deferred` and the change-entry.

## Open questions

None — all decisions are mechanical per CC requirements.
