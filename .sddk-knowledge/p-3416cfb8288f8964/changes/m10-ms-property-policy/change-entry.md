# Change: m10-ms-property-policy — property feed policy single owner

## Subject

- **Cycle**: m10-ms-property-policy
- **Path**: A-min (T0+T2+T4-smoke)
- **Base SHA**: `1611064681fe5ea69bac0ed0a5f9fda76e931ced`
- **Head SHA**: `95c998e7d3ea6f3ee2ad0c160a54b07f08554ceb`
- **Status**: completed

## Files changed

- `crates/chronos-domain/src/property.rs`: added `evaluate_feed` / `evaluate_feed_violation` (single owner of feed-level emptiness policy) + 2 fixture tests.
- `crates/chronos-capture/src/observation_log.rs`: session evaluators delegate to domain policy.
- `crates/chronos-capture/src/state_recorder.rs`: delegates to domain policy.

## Evidence

Acceptance F6 (single policy owner in domain) and F10 (shared fixture
contract) verified in verify-report.md. Closes architecture slice S3 /
MS-PROPERTY-POLICY from the m10 product-evolution roadmap; unblocks
MS-INV-ORCHESTRATOR dependency chain.

## Cross-check

- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p chronos-domain -p chronos-capture -p chronos-services --all-targets -- -D warnings`: passed.
- T2 + workspace lib (941 ok) + chronos-native serial (103 ok): passed.
- T4-smoke e2e_connectivity: passed 1/1.
