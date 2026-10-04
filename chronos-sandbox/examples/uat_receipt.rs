//! Receipt probe for the OTLP UAT executors (M6 and M7).
//!
//! ## Why this exists
//!
//! `CERTIFICATION.md` §4 asks each certificate to carry "fixture, command,
//! duration, result, GitHub Actions run **or ruta de recibo inmutable**", and
//! the four UAT executors below were the reason no M6/M7 certificate could be
//! written honestly: the tests that run them prove the outcome, but once the
//! test run is gone the only record is a claim in a document. A claim is not
//! evidence.
//!
//! ## What it does and does not assert
//!
//! It prints, and asserts nothing about the values. These four are pure
//! functions over fixed fixtures, so a green run is reproducible and a red one
//! is a real signal — but the probe still treats the printed block as a
//! receipt to be filed, not as a test. The assertion lives in the tests that
//! already exist (`otlp_cross_service`, `otlp_cost_memory_collision`,
//! `cost_memory_uat_wire`), and duplicating it here would give two places to
//! keep in sync for no extra coverage.
//!
//! ## Why M6 prints Debug and M7 prints JSON
//!
//! Not an inconsistency in effort — a consequence of a decision already made.
//! `UatResult` deliberately does not derive `serde` (the domain stays free of
//! wire-shape concerns), which is exactly why `chronos_mcp::cost_memory_wire`
//! exists as a separate `UatResultWire` adapter with `run_uat_m7_01_as_json`.
//! **M7 has a wire adapter in product, so M7 emits the canonical JSON. M6 has
//! none, and building `cross_service_wire` would be a product change smuggled
//! into a certification task.** The gap is filed as a limitation in the M6
//! certificates rather than papered over.

use chronos_domain::otlp::cross_service::{run_uat_m6_01, run_uat_m6_02};
use chronos_mcp::cost_memory_wire::{run_uat_m7_01_as_json, run_uat_m7_02_as_json};

fn main() {
    println!("# UAT receipt — M6 (chronos_domain::otlp::cross_service, Debug)");
    println!();
    println!("## UAT-M6-01");
    println!("{:#?}", run_uat_m6_01());
    println!();
    println!("## UAT-M6-02");
    println!("{:#?}", run_uat_m6_02());
    println!();
    println!("# UAT receipt — M7 (chronos_mcp::cost_memory_wire, canonical JSON)");
    println!();
    println!("## UAT-M7-01");
    println!("{}", pretty(&run_uat_m7_01_as_json()));
    println!();
    println!("## UAT-M7-02");
    println!("{}", pretty(&run_uat_m7_02_as_json()));
}

fn pretty(r: &Result<serde_json::Value, String>) -> String {
    match r {
        Ok(v) => {
            serde_json::to_string_pretty(v).unwrap_or_else(|e| format!("<unserializable: {e}>"))
        }
        Err(e) => format!("<error: {e}>"),
    }
}
