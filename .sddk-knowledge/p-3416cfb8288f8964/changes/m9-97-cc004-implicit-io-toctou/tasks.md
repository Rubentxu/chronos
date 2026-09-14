# m9-97 Tasks: cc-004 implicit I/O TOCTOU falsification

1. Inspect the save path and redb 2.6.3 transaction tracker.
   - Confirm `begin_write()` precedes `begin_read()` in `ce_write.rs`.
   - Confirm `start_write_transaction()` serializes writers.
2. Reject the hypothesized interleaving as impossible under the pinned database
   model.
3. Revert the local experimental Rust refactor and its non-diagnostic race test
   before release.
4. Move cc-004 from active to terminated in `terms/index.md`, with the evidence
   rather than a claim of a behavior fix.
5. Run T0, workspace lib tests, manifest fixpoint, and vault-drift gates.
6. Produce normal cycle artifacts, release `v0.7.99`, archive, push, and delete
   the cycle branch.
