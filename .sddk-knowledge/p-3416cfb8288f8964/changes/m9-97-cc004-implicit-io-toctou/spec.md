# m9-97 Spec: cc-004 implicit I/O TOCTOU falsification

## R1 — writer serialization is the governing invariant

`save_bundle_record_and_events` SHALL retain its existing order:
`begin_write()` before the independent `begin_read()` used to collect prior
keys.

**Scenario R1.1**: Given a save has acquired a redb `WriteTransaction`, when
another thread calls `begin_write()`, then redb blocks that second writer until
the first write transaction ends.

Evidence: redb 2.6.3 `TransactionTracker::start_write_transaction()` loops on
`live_write_transaction.is_some()` before assigning the new transaction ID.

## R2 — no unsound refactor ships

The experimental table-scoped helper refactor and concurrent-resave test SHALL
not ship because their premise is false. The final m9-97 tree SHALL have no
net Rust source delta from m9-96.

## R3 — finding closure is evidence-bound

The terms index SHALL mark cc-004 terminated as a false positive, citing the
write-before-read order in `ce_write.rs` and redb's enforced single-writer
transaction model.

## R4 — verification

T0 formatting and clippy checks, the existing workspace lib suite, the manifest
fixpoint check, and the vault drift sweep SHALL pass.