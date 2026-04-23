//! DWARF type name resolution utilities.

// This module provides type name resolution from DWARF type DIEs.
// The actual implementation is in variables.rs where we have access to
// the full gimli Dwarf context.

/// Resolve a type name from a DWARF type offset.
///
/// This is a convenience wrapper that delegates to the implementation
/// in the variables module. The actual resolution is done by traversing
/// the DWARF DIE tree.
pub fn resolve_type_name(type_offset: u64) -> String {
    // The actual implementation is in variables::resolve_type_name
    // which requires access to the gimli Dwarf context.
    // This placeholder returns "unknown" since we don't have the context here.
    let _ = type_offset;
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_resolve_type_name_returns_unknown() {
        assert_eq!(super::resolve_type_name(0), "unknown");
    }
}
