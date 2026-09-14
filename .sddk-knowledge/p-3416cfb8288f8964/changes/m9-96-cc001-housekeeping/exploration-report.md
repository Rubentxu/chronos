# Exploration Report — m9-96: cc-001 god-module housekeeping

## Context

`cc-001-god-module` was opened in m9-04 as a P2 MEDIUM coupling debt
finding: `counterexample_storage.rs` at 2556 lines with 5 distinct
concerns. The finding was tracked in
`.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` under "Active
terms" → "Debt findings from m9-04".

The 5 concerns were resolved across 5 cycles:

| Cycle | Concern | File | Approach |
|---|---|---|---|
| m9-84 | keys-split | `counterexample_storage.rs` | Extracted `ce_chunk_keys.rs` (key encoding helpers) |
| m9-85 | impl-split | `counterexample_storage.rs` | Extracted `ce_read.rs` + `ce_write.rs` + `ce_test_hooks.rs` (impl methods) |
| m9-86 | types-split | `counterexample_storage.rs` | Extracted `ce_types.rs` (6 pub types + helpers) |
| m9-87 | schema-split | `counterexample_storage.rs` | Extracted `ce_schema.rs` (table defs + chunk constants) |
| m9-94 | storage test-split | `counterexample_storage.rs` | Extracted `ce_storage_tests.rs` (1771 lines of tests) |
| m9-95 | services test-split | `counterexample.rs` | Extracted `ce_services_tests.rs` (2169 lines of tests) |

(The original 5 concerns were keys/records/persistence/schema-version/v2-legacy;
m9-94 + m9-95 went beyond and split the test code monolith too.)

After all 6 cycles: both files are split into focused sibling modules.
The finding is **fully resolved** but remains formally open in
`terms/index.md`.

## Current state

`counterexample_storage.rs`:
- 1960 → 191 lines (post-m9-94; production code only)
- Test code: `ce_storage_tests.rs` (1781 lines)

`counterexample.rs`:
- 3955 → 1793 lines (post-m9-95; production code only)
- Test code: `ce_services_tests.rs` (2175 lines)

Active debt findings remaining from m9-04:
1. ~~`cc-001-god-module`~~ → will be closed by m9-96
2. `cc-004-implicit-io-toctou` (P3 LOW; deferred to m10+)

Active non-debt findings (per `terms/index.md`):
- `FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL` (m9-74, m9+ backlog; known pre-existing flake per AGENTS.md §6.5)
- `FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION` (m9-71, m9+ backlog; closed by CC#4 fixpoint tool in m9-76 but the chain-tension observation remains as a non-blocking note)

## Why m9-96 is the natural next cycle

The m9-95 closure handoff explicitly recommended m9-96:

> "m9-96 candidates (from m9-95 carry-forward + cc-001 housekeeping):
> 1. **cc-001-god-module housekeeping** (B-direct vault): mark
>    `cc-001-god-module` finding as CLOSED in
>    `.sddk-knowledge/p-3416cfb8288f8964/maintenance/active-findings.md`
>    now that production + test halves (storage + services) are all
>    split. **Strong candidate for m9-96**: small vault-only cycle
>    (A-lite scope), no Rust touched."

The finding has been fully resolved; only the bookkeeping (moving the
row from active to terminated) remains.

## Approach

A-lite vault-only cycle. No Rust touched. Three changes:

1. **Move `cc-001-god-module` row** from the "Active terms" →
   "Debt findings from m9-04" table (line 36) to the "Terminated
   terms" table (after line 122).
2. **Update `findings_closed`** in the m9-96 apply-checkpoint.json
   to include `cc-001-god-module`.
3. **Standard cycle artifacts + archive + handoff** following the
   established pattern (m9-94 + m9-95 cycle structure).

## Risk analysis

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `terms/index.md` markdown breaks | very low | low | `bash scripts/check_vault_drift.sh` validates the table structure (CC#39 Part C) |
| cycles/index.md Total cycles mismatches | low | medium | CC#39 Part C validates the count; bump Total 95 → 96 |
| Archive-manifest SHA-256 fixpoint breaks | very low | low | `python3 scripts/regen_manifest_index_shas.py` regenerates; --check verifies |
| Cargo clippy breaks | very low | low | Nothing changed in Rust; T0 sanity check confirms |

## Recommendation

Proceed with m9-96 as A-lite vault-only cycle. Smallest possible
next cycle; sets up m10+ backlog clean.

## Out-of-scope (deferred to future cycles)

- cc-004-implicit-io-toctou (P3 LOW; deferred to m10+): explicit-IO
  refactor across chronos-services / chronos-store / chronos-mcp.
  First m10 cycle.
- FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION: the chain
  tension is a non-blocking observation; CC#4 fixpoint tool already
  handles it. Not actionable in chronos scope.
- FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL: pre-existing flake
  (per AGENTS.md §6.5). Not actionable.
