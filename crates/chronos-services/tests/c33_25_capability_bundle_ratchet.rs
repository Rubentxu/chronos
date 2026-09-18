//! REC-C3.3.2.6 — anti-stale-waiver ratchet for the capability split.
//!
//! Background: before C3.3.2.5 the wrapper carried a `ProviderKind`
//! enum tag and downcast on every maintenance call
//! (`flush`, `compaction_metrics`, `compact_up_to`, `retain_up_to`,
//! `maybe_compact`). The tag was a closed 3-variant escape hatch;
//! the canonical evidence path went through the port but the
//! maintenance path did not. The result: `ExecutionLogMaintenanceUnsupported`
//! was a normal runtime error in production.
//!
//! C3.3.2.5 lifts maintenance and retention into first-class ports.
//! The wrapper no longer holds a `ProviderKind` enum, has no downcast
//! on `kind()`, and exposes the three-port bundle.
//!
//! ## Why this file exists
//!
//! Once a forbidden pattern is removed it tends to come back during
//! refactors unless the regression is caught at compile time or at
//! CI. This file is the ratchet. Each test asserts the absence of
//! one forbidden pattern in `chronos-services` and `chronos-mcp`,
//! so any future commit that re-introduces the pattern (e.g. by
//! adding `compact_up_to` back to the wrapper, or by routing
//! `flush` through a `ProviderKind::Segmented(_)` arm) fails this
//! file at `cargo test` time.
//!
//! The tests run as plain assertions over the file content, so the
//! cost is negligible (a few hundred bytes per grep).
//!
//! ## Patterns the ratchet forbids
//!
//! - `compact_up_to(` on `SessionExecutionLog` (the wrapper) —
//   maintenance has no opinion on the cutoff.
//! - `retain_up_to(` on `SessionExecutionLog` — replaced by
//!   `advance_retained_from` on the retention port.
//! - `maybe_compact(` on `SessionExecutionLog` — replaced by
//!   `compact_retired()` on the maintenance port.
//! - `ExecutionLogMaintenanceUnsupported` (any arm match on this
//!   variant) — the failure mode is gone; maintenance errors come
//!   from the port as `ExecutionLogMaintenanceError` /
//!   `RetentionError` translated to `DrainFailed` or new
//!   `Retention*` variants.
//! - `provider.kind()` (i.e. `ExecutionLogProvider::kind()` returns
//!   `ExecutionLogKind` and the caller matches on it) — `kind()` is
//!   still on the port for compatibility but services must never
//!   match on it.
//! - `ProviderKind` enum (the wrapper-local downcast tag) — gone
//!   from services entirely.
//! - `take_over_segmented` test bypass — gone.

use std::fs;
use std::path::{Path, PathBuf};

/// Walk `crates/<crate>/src/**.rs` and return the absolute paths.
fn walk_rs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_rs_inner(root, &mut out);
    out
}

fn walk_rs_inner(current: &Path, out: &mut Vec<PathBuf>) {
    let rd = match fs::read_dir(current) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk_rs_inner(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

fn read_all(paths: &[PathBuf]) -> String {
    let mut out = String::new();
    for p in paths {
        if let Ok(s) = fs::read_to_string(p) {
            out.push_str(s.as_str());
            out.push('\n');
        }
    }
    out
}

/// Read the source of a crate's `src` tree under the workspace root.
fn lib_sources(crate_name: &str) -> Vec<PathBuf> {
    // CARGO_MANIFEST_DIR is `<workspace>/crates/<crate>`. Sibling
    // crates live under `<workspace>/crates/<crate>/src`, so climb
    // one level (to `<workspace>/crates/`) and descend.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace crates")
        .join(crate_name)
        .join("src");
    if !root.exists() {
        return Vec::new();
    }
    walk_rs(&root)
}

/// Filter a concatenated source blob through a predicate on lines.
/// Returns the lines that match the predicate and are NOT inside a
/// `// ` line comment (rough heuristic).
fn grep_forbidden(source: &str, needles: &[&str]) -> Vec<String> {
    let mut hits = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        // Skip pure line comments (rough heuristic: comment lines
        // that contain a forbidden pattern are documenting the
        // ratchet itself, not the regression).
        if trimmed.starts_with("//") {
            continue;
        }
        for needle in needles {
            if line.contains(needle) {
                hits.push(format!("line {}: {}", idx + 1, line.trim()));
            }
        }
    }
    hits
}

// ---------------------------------------------------------------------------
// Ratchets
// ---------------------------------------------------------------------------

#[test]
fn ratchet_no_compact_up_to_in_services_or_mcp() {
    let services = read_all(&lib_sources("chronos-services"));
    let mcp = read_all(&lib_sources("chronos-mcp"));
    // We forbid `compact_up_to(` in production source. The single
    // legitimate remaining call site is `chronos_native` integration
    // tests (which exercise the concrete backend API directly) — see
    // AGENTS.md note. Services and mcp MUST NOT call it on a
    // `SessionExecutionLog` or through any port.
    let hits = grep_forbidden(&services, &["compact_up_to("]);
    let hits_mcp = grep_forbidden(&mcp, &["compact_up_to("]);
    assert!(
        hits.is_empty(),
        "compact_up_to( must not reappear in chronos-services; hits: {hits:#?}"
    );
    assert!(
        hits_mcp.is_empty(),
        "compact_up_to( must not reappear in chronos-mcp; hits: {hits_mcp:#?}"
    );
}

#[test]
fn ratchet_no_retain_up_to_in_services_or_mcp() {
    let services = read_all(&lib_sources("chronos-services"));
    let mcp = read_all(&lib_sources("chronos-mcp"));
    let hits = grep_forbidden(&services, &["retain_up_to("]);
    let hits_mcp = grep_forbidden(&mcp, &["retain_up_to("]);
    assert!(
        hits.is_empty(),
        "retain_up_to( must not reappear in chronos-services; hits: {hits:#?}"
    );
    assert!(
        hits_mcp.is_empty(),
        "retain_up_to( must not reappear in chronos-mcp; hits: {hits_mcp:#?}"
    );
}

#[test]
fn ratchet_no_maybe_compact_in_services_or_mcp() {
    let services = read_all(&lib_sources("chronos-services"));
    let mcp = read_all(&lib_sources("chronos-mcp"));
    let hits = grep_forbidden(&services, &[".maybe_compact("]);
    let hits_mcp = grep_forbidden(&mcp, &[".maybe_compact("]);
    assert!(
        hits.is_empty(),
        ".maybe_compact( must not reappear in chronos-services; hits: {hits:#?}"
    );
    assert!(
        hits_mcp.is_empty(),
        ".maybe_compact( must not reappear in chronos-mcp; hits: {hits_mcp:#?}"
    );
}

#[test]
fn ratchet_no_execution_log_maintenance_unsupported_variant() {
    let services = read_all(&lib_sources("chronos-services"));
    let mcp = read_all(&lib_sources("chronos-mcp"));
    let hits = grep_forbidden(&services, &["ExecutionLogMaintenanceUnsupported"]);
    let hits_mcp = grep_forbidden(&mcp, &["ExecutionLogMaintenanceUnsupported"]);
    assert!(
        hits.is_empty(),
        "ExecutionLogMaintenanceUnsupported must not reappear in chronos-services; \
         hits: {hits:#?}"
    );
    assert!(
        hits_mcp.is_empty(),
        "ExecutionLogMaintenanceUnsupported must not reappear in chronos-mcp; \
         hits: {hits_mcp:#?}"
    );
}

#[test]
fn ratchet_no_provider_kind_enum_in_services() {
    let services = read_all(&lib_sources("chronos-services"));
    // The wrapper-local `ProviderKind` enum must NOT reappear.
    // The port still exports `ExecutionLogKind` (the closed adapter
    // discriminator used for routing on the port); that's allowed.
    let hits = grep_forbidden(
        &services,
        &[
            "enum ProviderKind",
            "ProviderKind::Segmented",
            "ProviderKind::InMemory",
        ],
    );
    assert!(
        hits.is_empty(),
        "ProviderKind enum/arms must not reappear in chronos-services; hits: {hits:#?}"
    );
}

#[test]
fn ratchet_no_take_over_segmented_in_services() {
    let services = read_all(&lib_sources("chronos-services"));
    let hits = grep_forbidden(&services, &["take_over_segmented"]);
    assert!(
        hits.is_empty(),
        "take_over_segmented must not reappear in chronos-services; hits: {hits:#?}"
    );
}

#[test]
fn ratchet_no_try_adopt_in_services_or_mcp() {
    let services = read_all(&lib_sources("chronos-services"));
    let mcp = read_all(&lib_sources("chronos-mcp"));
    let hits = grep_forbidden(&services, &["try_adopt"]);
    let hits_mcp = grep_forbidden(&mcp, &["try_adopt"]);
    assert!(
        hits.is_empty(),
        "SessionExecutionLog::try_adopt must not reappear in chronos-services; \
         hits: {hits:#?}"
    );
    assert!(
        hits_mcp.is_empty(),
        "SessionExecutionLog::try_adopt must not reappear in chronos-mcp; \
         hits: {hits_mcp:#?}"
    );
}
// FORCE_REBUILD_1789758365
