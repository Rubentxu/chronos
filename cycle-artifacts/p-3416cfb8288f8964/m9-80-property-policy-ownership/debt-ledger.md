# Debt Ledger — m9-80-property-policy-ownership

Assessed 2026-09-29 during reconciliation of the abandoned PAUSED cycle.
Surface: cycle diff `82e219f..ccf8811` (18 files, +1000/-450).

## RETRACTED — both original findings were measurement errors

This ledger originally recorded two findings. **Both were wrong**, and both were
caused by the same faulty method: I measured function length by scanning from one
`fn` line to the next `fn` line, which silently spans `impl` blocks and any
`fn` whose body contains a nested block. That produced a fictitious 361-line
function.

Re-measured with true brace matching (count `{`/`}` depth to find where each
function actually closes), over the same 779 production lines:

| | Original claim | Corrected measurement |
|---|---|---|
| longest production fn | 361 lines (`compare_ord`) | **50 lines** (`parse_invariant`, 632-681) |
| `compare_ord` | "361-line function" | **9 lines** (245-254) |
| `Property::evaluate` | not identified | 85 lines (261-345) |

Full production ranking: `parse_invariant` 50, `invariant_text` 25,
`unescape` 20, `compare_num` 18, `parse_value` 16, `outcome_for` 16,
`glob_match` 15, `parse_op` 11, `compare_ord` 9. **No production function
exceeds 50 lines.** There is no oversized-function debt in this surface.

### `m980-D1` — RETRACTED, does not exist

Claimed a 361-line `compare_ord`. It is 9 lines. The apparent size came from
lines 256+ belonging to `impl Property { ... }`, which my scanner counted as
part of the preceding free function. **No finding.**

### `m980-D2` — RETRACTED, not a defect

Claimed a low-severity `unwrap()` at line 432. It is provably safe: the
`Contains | Matches` arm returns early when `observations.is_empty()`
(lines 411-416), so by line 425 the slice is non-empty and `.last()` is `Some`.
`clippy -D warnings` is clean, which independently corroborates this. **No
finding.**

## Ratchet

No regression attributable to this cycle. It **reduced**
`chronos-services/hypothesis_test.rs` by 239 lines and moved policy ownership
into the domain layer, which is the intended direction.

## Verdict

**Zero debt findings in this surface.** The cycle's own code is well-factored:
13 production functions, longest 50 lines, one production `unwrap()` that is
provably unreachable-when-empty, 779 production lines against 1855 total (the
remainder is tests). 964 tests pass, `cargo fmt --check` and
`cargo clippy -D warnings` are clean.

The original two findings were artefacts of my own tooling, not properties of
the code. Recording them as debt would have put a false claim in a canonical
ledger and created a phantom follow-up cycle.

## Process note

The `debt-severity-assigned` and `debt-priority-assigned` gate receipts
(`gate-debt-severity-assigned-f0782ea44e4b8242-1`,
`gate-debt-priority-assigned-f0782ea44e4b8242-1`) recorded these two false
findings. The receipts are historical and immutable; this ledger supersedes
their content. Nothing about the release decision changes — the gates
supported "no pending effect", which remains true.
