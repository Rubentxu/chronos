# m9-97 Exploration: cc-004 implicit I/O TOCTOU falsification

## Original claim

`save_bundle_record_and_events` opened an independent read transaction to find
prior chunk keys after acquiring its write transaction. cc-004 treated that as
a classic TOCTOU race: another writer might commit after the read and before
the local delete/write sequence.

## Falsification

The required interleaving cannot occur with the project's pinned redb 2.6.3:

- In `crates/chronos-store/src/ce_write.rs`, the code calls `begin_write()` at
  lines 84-87 before it opens `begin_read()` at lines 103-106.
- In redb 2.6.3, `Database::begin_write()` calls
  `TransactionTracker::start_write_transaction()`
  (`db.rs:1025-1033`).
- `start_write_transaction()` waits while
  `state.live_write_transaction.is_some()`
  (`transaction_tracker.rs:117-128`), then marks the one live writer.
- A concurrent writer therefore waits at `begin_write()` until the current
  save commits or aborts. It cannot commit between this save's read and its
  chunk cleanup.

The read transaction captures the committed state available when the active
writer began its cleanup. Since no competing writer can commit while that
writer remains live, its key list cannot become stale due to a writer.

## Experiment rejected

A local refactor moved key collection onto a write-table handle and added two
threaded re-save test cases. The test passed, but that only demonstrated redb's
writer serialization. It did not reproduce a pre-fix race. The change was
reverted before publication because preserving behavior with an explanatory
finding closure is the smaller, correct result.

## Conclusion

cc-004 is a false positive, not a deferred code defect. No Rust change is
needed. The applicable future trigger is a change to the transaction ordering
or to the redb single-writer model, at which point this assessment must be
revisited.
