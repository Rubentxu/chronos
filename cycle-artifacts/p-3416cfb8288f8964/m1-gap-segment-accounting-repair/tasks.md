# M1 — Gap segment accounting repair (tasks)

**Cycle**: `m1-gap-segment-accounting-repair`
**Companion**: `proposal.md`

Ordered so each GREEN leaves the tree green.

## T0 — characterization (RED)

`crates/chronos-log/tests/m1_gap_segment_accounting.rs`:
`char_header_counts_entries_but_replay_counts_records` pins the divergence
(header declares 4 entries; replay counts 3 records; reopen fails with
`RecordCountMismatch`). Committed RED, stays in the tree as documentation.

## T1 — one unit for writer/header/replay

1. `crates/chronos-log/src/segment.rs`:
   - rename `SegmentMetadata::record_count` → `entry_count`.
   - rename the `write_segment` parameter `record_count` → `entry_count`.
   - update the format doc table + the payload paragraph: the count is of
     `SegmentEntry` items; a `Gap` is one entry that spans a seq range.
   - `read_header` / `read_segment` read into `entry_count`.
2. `crates/chronos-log/src/replay.rs`:
   - count **every** entry (records + gaps), not records only.
   - compare to `header.entry_count`; error text says "entries".
   - `RecordCountMismatch` → `EntryCountMismatch` (declared/actual of
     entries).
3. `crates/chronos-log/src/segmented.rs::flush_inner`: pass
   `entry_count = buffer.len()` (unchanged quantity, honest name).

## T2 — GAP-PERSIST-1..6 (GREEN)

Extend `m1_gap_segment_accounting.rs`:

- **GAP-PERSIST-1** records + Gap + records → flush → reopen → same
  sequence and same Gap.
- **GAP-PERSIST-2** Gap via `record_gap` → reopen valid.
- **GAP-PERSIST-3** Gap via memory-budget overflow → reopen valid.
- **GAP-PERSIST-4** `header.entry_count == entries.len()` exactly.
- **GAP-PERSIST-5** Gap 100..=199 occupies one entry, preserves the range,
  next appended seq is 200.
- **GAP-PERSIST-6** reopen → read crossing the gap → `gaps` non-empty and
  the surviving seq space is contiguous (never a silent hole).

## T3 — C1.8 UAT GREEN

`chronos-sandbox/tests/rec_c1_8_uat_c1_03_forced_gap.rs`:
- remove `#[ignore]` from `uat_rec_c1_03_forced_gap_reports_gap_detected`.
- simplify the fixture back to the direct `record_gap` path (the log now
  reopens), keeping the clean-session negative arm.
- run with `CHRONOS_MCP_PATH` set; both tests GREEN.

## T4 — gates + close

- T0 fmt + clippy `-D warnings`.
- T3 split (workspace `--lib` excluding sandbox/e2e/native +
  `chronos-native --lib -- --test-threads=1`).
- T4-smoke: `rec_c1_8_uat_c1_03_forced_gap` (2/2) + `e2e_connectivity`
  (1/1) + `rec_c1_8_dual_truth_closeout` (3/3) + `rec_c1_8_uat_c1_01_exact`
  (2/2).
- regen archive-manifest SHAs; merge `--no-ff`; annotated tag
  `m1-gap-segment-accounting-repair`; `archive_status = "ready"`.
- Update the REC-C1.8 artifact: FIND-C1.8-01 `open_deferred` → `closed`.
