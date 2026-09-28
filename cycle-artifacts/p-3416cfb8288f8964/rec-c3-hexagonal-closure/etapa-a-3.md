# REC-C3-hexagonal-closure · Etapa A.3

## Scope

Wire `NativeProbeController` port (added in A.1) through `LiveProbeSession`,
completing the services→native dependency inversion for the probe pathway.

## Commit

`85d621fb` — feat(rec-c3.3.4-native): etapa A.3 rewire LiveProbeSession to
NativeProbeController port

## Changes

| File | Change |
|---|---|
| `crates/chronos-domain/src/ports/probe.rs` | Added `start`, `clone_resolver_pipeline`, `resolve_context` to `NativeProbeController` trait. `clone_resolver_pipeline` now returns `ResolverPipeline` directly (Clone), not `Option<Arc<dyn SemanticResolver>>` |
| `crates/chronos-domain/tests/probe_ports.rs` | Updated `StubNativeController` for new method shapes |
| `crates/chronos-native/src/native_probe_controller.rs` | Added `start`, `clone_resolver_pipeline`, `resolve_context` impls |
| `crates/chronos-services/src/probe.rs` | `LiveProbeSession.backend` → `controller: Box<dyn NativeProbeController>`. 6 backend call sites rewired through controller |
| `crates/chronos-services/src/observe.rs` | `register_fake_probe_session` updated to use controller |
| `crates/chronos-mcp/src/server.rs` | 2 test fixtures rewired to use `NativeProbeControllerImpl`. **Drift fix bundled**: added `probe_advance`/`probe_step` to `ALL_TOOL_NAMES` and bumped hardcoded 61 → 63 in 3 places (Tren B introduced these tools but did not update `ALL_TOOL_NAMES`) |

## Drift fix (Tren B bug, not introduced by A.3)

Tren B (REC-C3.3.3) added `probe_advance` and `probe_step` as new `#[tool]`
router entries but did NOT update `ALL_TOOL_NAMES`, so the
`toolset_sync_check::all_tool_names_matches_router` tripwire would have caught
it. Hardcoded assertions in `cap_discovery_tests` (the function name itself
contains "61", comments, and `assert_eq!(.., 61)`) were also stale. All bumped
to 63.

This is a Tren B pre-existing drift, NOT introduced by the A.3 rewire.
Confirmed by:

  1. `git diff origin/main..HEAD -- crates/chronos-mcp/src/server.rs` shows
     the router additions (`probe_advance`/`probe_step`) are part of Tren B's
     slice G, not my A.3 work.
  2. `git show origin/main:crates/chronos-mcp/src/server.rs | grep -c '"'`
     yields 770 entries in ALL_TOOL_NAMES; my branch has 783 (delta +13 from
     other Tren B tools; only `probe_advance`+`probe_step` were missing from
     `ALL_TOOL_NAMES`).
  3. The 2 failing tests are pure const-assertion failures — they would have
     failed before any A.3 work was done; they were just collateral-discovered
     when I first ran `cargo test -p chronos-mcp --lib` on the integration
     branch.

The clean separation: A.3 work changed only the `controller` field and call
sites; the drift fix is independent and would be needed regardless of when
REC-C3.3.3 was merged. Documenting in the same commit because the diff is
interleaved (same file, same module).

## Evidence (pre-commit gate)

| Gate | Result |
|---|---|
| `cargo check --workspace --all-targets` | OK |
| `cargo fmt --all -- --check` | OK |
| `cargo clippy --workspace --all-targets -- -D warnings` | OK |
| `cargo test -p chronos-domain --test probe_ports` | 12/12 OK |
| `cargo test -p chronos-native --lib -- --test-threads=1` | 109/109 OK |
| `cargo test -p chronos-services --lib` | 391/391 OK |
| `cargo test -p chronos-mcp --lib` | 109/109 OK (was 107/109 before drift fix) |

Ptrace tests run with `--test-threads=1` per AGENTS.md §6.5.

## Audit-grounded decisions

  * **No `backend() -> &dyn ProbeBackend` accessor** on the controller port
    (audit REC-C3.3.4 §4.5 S4 explicit anti-pattern).
  * **No facade** that re-exports backend methods on the port; every method
    on the port has a single canonical capability forward.
  * **Honest placeholder** for `resolver_pipeline()` / `clone_resolver_pipeline()`
    returning `None` / empty pipeline (audit §6.2: documented limitation,
    REC-C4 follow-up because `ResolverPipeline` is a concrete struct, not a
    trait object).
  * **`pause_reason`** is a placeholder string until kernel event threading is
    added (documented in source).

## Binary artifacts

  * Path: `/var/home/rubentxu/cargo-targets/debug/chronos-mcp`
    (note: `CARGO_TARGET_DIR=/var/home/rubentxu/cargo-targets`; local
    `target/debug/chronos-mcp` is **stale**, do not use).
  * SHA256: `56b75707ef6dd3366a492e3934c734aad8f5d1f05504da0981aac543514ff88c`
  * mtime: 2026-09-20 12:37:44

## State after A.3

  * `LiveProbeSession` holds `Box<dyn NativeProbeController>` (port, not concrete).
  * `services` no longer reaches into `chronos_native::probe_backend` directly.
  * Etapa A complete. Next: Etapa B (services→store ports: `DiffSource`,
    `ExplainSource`, `CompareSource`, `LifecycleStore`).
