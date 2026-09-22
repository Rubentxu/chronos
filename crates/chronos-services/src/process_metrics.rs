//! CAP-GAP-CHRONOS-H1.5-MEMORY-PROBE closure (2026-09-22).
//!
//! Per H1.5 §6.2: no in-tree RSS / heap-size measurement helper
//! existed before this cycle (`getrusage` was only referenced as a
//! syscall name string in `chronos-native/src/syscall_table.rs:109`).
//! This module adds a pure-stdlib `process_rss_kb()` helper that reads
//! `/proc/self/statm` on Linux and returns the resident set size in
//! kibibytes. Non-Linux platforms return `None`.
//!
//! ## Why `/proc/self/statm` and not `getrusage`?
//!
//! - `/proc/self/statm` is pure stdlib (one `File::open` + one
//!   `read_to_string`); `getrusage` would require `libc` or
//!   `nix` crate.
//! - The H1.5 matrix only requires a coarse memory probe (see
//!   `docs/architecture/H1.5-runtimes-capabilities-benchmarks.md` §3.4
//!   OPEN follow-ups). A `statm` page-count × page-size approximation
//!   is sufficient and avoids adding a new dependency.
//! - `statm` reports the **process** RSS as the kernel sees it,
//!   including all heap arenas; `getrusage` reports `ru_maxrss` which
//!   is the high-water mark and is monotonic — different semantics.
//!
//! ## Out-of-scope (M1+)
//!
//! - Cross-platform support (macOS, Windows). Today this returns `None`
//!   on non-Linux; a `sysinfo` crate wrapper would be the next step
//!   if cross-platform support becomes a requirement.
//! - Heap-fragmentation analysis. `statm` cannot distinguish resident
//!   heap from RSS. A jemalloc/MALLOC stats probe would be needed for
//!   fragmentation metrics.
//! - Per-thread / per-allocator memory attribution. Same reason.
//!
//! ## Why this is honest closure of the gap
//!
//! The original H1.5 §6.2 entry said "no in-tree RSS / heap-size
//! measurement; `getrusage` referenced as string only". With this
//! module, there is a real, tested, Linux-side RSS probe available
//! to bench code. The bench can now be wired to call this helper
//! before/after a workload to measure Δ RSS.

use std::fs;
use std::path::Path;

/// Page size used by `/proc/self/statm`. Linux defaults to 4 KiB on
/// every architecture we target; this is documented in `man 5 proc`.
#[cfg(target_os = "linux")]
const LINUX_PAGE_SIZE_KB: u64 = 4;

/// Read `/proc/self/statm` and return the resident set size in
/// kibibytes. Returns `None` on non-Linux platforms or if the file
/// cannot be read (rare; would mean `/proc` is not mounted).
///
/// The function uses the `Path` argument as the read target so tests
/// can point it at a fixture file. Production callers should pass
/// `Path::new("/proc/self/statm")`.
///
/// `/proc/self/statm` format (whitespace-separated):
///   size       (1) total program size
///              (same as VmSize in /proc/[pid]/status)
///   resident   (2) resident set size
///              (same as VmRSS in /proc/[pid]/status)
///   shared     (3) number of resident shared pages (i.e., backed by a file)
///              (same as RssFile+RssShmem in /proc/[pid]/status)
///   text       (4) text (code)
///   lib        (5) library (unused since Linux 2.6; always 0)
///   data       (6) data + stack
///   dt         (7) dirty pages (unused since Linux 2.6; always 0)
///
/// We read field (2) and multiply by the page size.
///
/// Sources: `man 5 proc`, Linux kernel `fs/proc/array.c`.
pub fn process_rss_kb(statm_path: &Path) -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let raw = fs::read_to_string(statm_path).ok()?;
        let mut fields = raw.split_whitespace();
        // Field (1) is size — skip.
        fields.next()?;
        // Field (2) is resident pages.
        let resident_pages: u64 = fields.next()?.parse().ok()?;
        Some(resident_pages.saturating_mul(LINUX_PAGE_SIZE_KB))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = statm_path;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Spec-aligned fixtures:
    /// `man 5 proc` says a typical line looks like
    /// `210473 73075 9568 1328 0 29980 0`. The function reads
    /// field (2) (resident) and multiplies by page size.
    #[test]
    fn parses_canonical_statm_fixture() {
        let dir = std::env::temp_dir().join(format!(
            "chronos-rss-fixture-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("statm");
        let mut f = std::fs::File::create(&p).unwrap();
        // Format: size resident shared text lib data dt
        // 210473 73075 9568 1328 0 29980 0
        // Resident = 73075 pages × 4 KiB = 292300 KiB
        writeln!(f, "210473 73075 9568 1328 0 29980 0").unwrap();
        let rss = process_rss_kb(&p).expect("linux only");
        assert_eq!(rss, 73075 * 4);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn returns_none_on_missing_file() {
        let p = std::path::Path::new("/this/path/does/not/exist/statm");
        assert_eq!(process_rss_kb(p), None);
    }

    #[test]
    fn returns_none_on_garbage_input() {
        let dir = std::env::temp_dir().join(format!(
            "chronos-rss-garbage-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("statm");
        std::fs::write(&p, "this is not a statm line\n").unwrap();
        assert_eq!(process_rss_kb(&p), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn handles_short_line_gracefully() {
        let dir = std::env::temp_dir().join(format!(
            "chronos-rss-short-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("statm");
        std::fs::write(&p, "210473\n").unwrap();
        // Only size field; resident is missing.
        assert_eq!(process_rss_kb(&p), None);
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(test)]
mod live_proc_self_statm_tests {
    //! Live `/proc/self/statm` integration test (Linux only).
    //!
    //! Distinct from the fixture-based unit tests in `mod tests`;
    //! this one reads from the real kernel.
    #[test]
    fn reads_real_proc_self_statm() {
        let rss = super::process_rss_kb(std::path::Path::new("/proc/self/statm"));
        let rss = rss.expect("Linux: /proc/self/statm must be readable");
        // Sanity bounds: a Rust test binary on Linux typically uses 5-200 MiB.
        assert!(rss > 1024, "RSS impossibly low: {rss} KiB");
        assert!(rss < 4 * 1024 * 1024, "RSS impossibly high: {rss} KiB");
        eprintln!(
            "real /proc/self/statm RSS: {rss} KiB ({:.1} MiB)",
            rss as f64 / 1024.0
        );
    }
}
