//! Source location mapping using DWARF debug info.

use chronos_domain::trace::SourceLocation;

// Use addr2line's re-exported gimli for type compatibility
use addr2line::gimli;

/// Get source location for a program counter address using addr2line.
pub fn source_location(
    ctx: &addr2line::Context<gimli::EndianSlice<'_, gimli::RunTimeEndian>>,
    pc: u64,
) -> Option<SourceLocation> {
    // Find the frame containing this PC
    let location = ctx.find_location(pc).ok()??;

    // Build file path
    let file = location.file.map(|f| f.to_string());

    // Get line and column
    let line = location.line;
    let column = location.column;

    // Note: addr2line::Location doesn't have a function field directly
    // Function names come from the frame (but addr2line's API is different)
    let function = None;

    Some(SourceLocation {
        file,
        line,
        column,
        function,
        address: pc,
    })
}

#[cfg(test)]
mod tests {
    /// The DWARF reader over the test binary, built exactly once per process.
    ///
    /// Both the bytes and the reader are shared on purpose. The test binary is
    /// ~39 MB and `DwarfReader::new` parses its DWARF, so building one per test
    /// meant three full parses running in parallel with the ptrace tests, which
    /// starved them badly enough to make them time out.
    ///
    /// `DwarfReader` is not `Sync` — gimli keeps line caches in an
    /// `UnsafeCell` — so the shared reader lives behind a `Mutex`. `Send` is
    /// enough for that, and the lock also keeps the concurrent `find_location`
    /// calls that would otherwise be unsound.
    ///
    /// This is deliberately strict: the test build carries debuginfo, so a
    /// reader that cannot be built means the premise of these tests is gone and
    /// they should fail loudly rather than skip into a green.
    fn with_reader<T>(f: impl FnOnce(&super::super::DwarfReader<'static>) -> T) -> T {
        static READER: std::sync::OnceLock<std::sync::Mutex<super::super::DwarfReader<'static>>> =
            std::sync::OnceLock::new();
        let guard = READER
            .get_or_init(|| {
                let exe = std::env::current_exe().expect("the test binary path is known");
                let raw = std::fs::read(exe).expect("the test binary is readable");
                let bytes: &'static [u8] = Box::leak(raw.into_boxed_slice());
                std::sync::Mutex::new(
                    super::super::DwarfReader::new(bytes)
                        .expect("the test binary should carry DWARF"),
                )
            })
            .lock()
            .expect("the shared DWARF reader lock must not be poisoned");
        f(&guard)
    }

    /// Pins the graceful-degradation contract: an address with no debug info
    /// yields no location instead of panicking or inventing one.
    ///
    /// This test used to have an entirely empty body — only comments — while
    /// its name promised exactly this behaviour, and it referred to
    /// `find_location`, which is an addr2line internal rather than anything
    /// this module exposes. It could not fail and asserted nothing.
    #[test]
    fn source_location_is_none_for_an_address_without_debug_info() {
        with_reader(|reader| {
            assert!(
                reader.source_location(0xdead_beef).is_none(),
                "an address far outside the image has no debug info, so it must not resolve"
            );
        });
    }

    /// The same degradation must not depend on the particular address.
    ///
    /// `u64::MAX` is deliberately absent: addr2line 0.22.0 panics with
    /// "attempt to add with overflow" on that value while computing a section
    /// offset. That is upstream, not this crate, and it is parked as its own
    /// finding rather than hidden by quietly narrowing the range.
    #[test]
    fn source_location_is_none_across_addresses_without_debug_info() {
        with_reader(|reader| {
            for pc in [0xdead_beefu64, 0x7fff_ffff_ffff, 0xffff_ffff_0000_0000] {
                assert!(
                    reader.source_location(pc).is_none(),
                    "pc {pc:#x} has no debug info, so it must not resolve"
                );
            }
        });
    }

    /// And the positive side, so these tests are not only proving absence: the
    /// test binary's own image must carry real locations. Without this, an
    /// implementation returning `None` unconditionally would pass.
    #[test]
    fn source_location_resolves_for_an_address_inside_the_image() {
        with_reader(|reader| {
            // A short scan is enough: the reader is already indexed, and probing
            // a wide range here is what would dominate the suite's runtime.
            let resolved = (0..0x4_000)
                .step_by(0x400)
                .find_map(|pc| reader.source_location(pc).map(|l| (pc, l)));
            let (pc, location) =
                resolved.expect("a debug-built test binary must resolve some address");
            assert_eq!(
                location.address, pc,
                "the resolved address must be the one asked for"
            );
            assert!(
                location.line.is_some(),
                "a real location should carry a line number for pc {pc:#x}"
            );
        });
    }
}
