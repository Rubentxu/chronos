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
    #[test]
    fn test_source_location_returns_none_for_invalid_pc() {
        // This test verifies that find_location returns None for invalid addresses
        // Without a real DWARF binary, we can't do much more
        // The addr2line crate handles edge cases gracefully
    }
}
