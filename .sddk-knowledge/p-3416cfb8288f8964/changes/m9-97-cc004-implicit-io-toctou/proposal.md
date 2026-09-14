# m9-97 Proposal: cc-004 implicit I/O TOCTOU falsification

## Intent

Validate `cc-004-implicit-io-toctou`, a P3 LOW finding inherited from m9-04,
before spending an m10 code cycle on an alleged read-then-write race in
`SessionStore::save_bundle_record_and_events`.

## Finding

The alleged race is **not reachable** with redb 2.6.3:

1. `save_bundle_record_and_events` acquires `self.db().begin_write()` before it
   creates its independent `begin_read()` snapshot.
2. redb `Database::begin_write()` calls
   `TransactionTracker::start_write_transaction()`.
3. `start_write_transaction()` waits while `live_write_transaction.is_some()`
   (redb 2.6.3 `transaction_tracker.rs:117-128`).
4. Consequently, a second local redb writer cannot begin, much less commit,
   while the first save owns its write transaction and performs the separate
   read. Cross-process writers follow redb's single-writer transaction model.

Therefore no writer can commit between key discovery and deletion. The existing
v3/v2 cleanup sequence remains atomic relative to writers. The concurrency
regression test originally proposed for this cycle is invalid because it only
observes redb writer serialization, not a race.

## Scope

- Revert the unneeded local experimental Rust refactor before publication.
- Record the redb source evidence and close cc-004 as a falsified finding.
- Update terms, cycle artifacts, archive bindings, and handoff.

## Out of scope

- No production Rust behavior change.
- No redb upgrade or transaction-model change.
- No MCP or schema change.

## Acceptance criteria

1. The branch has no net Rust diff against its m9-96 base.
2. `cc-004-implicit-io-toctou` is moved from active to terminated with the
   redb single-writer evidence.
3. Vault gates and the cycle's T0 sanity checks are clean.
