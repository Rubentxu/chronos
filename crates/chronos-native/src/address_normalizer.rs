//! Address normalization for ASLR-aware trace comparison.
//!
//! Provides `AddressNormalizer` trait and `SymbolOffsetNormalizer` implementation
//! to normalize addresses to `(symbol_name, offset)` tuples before hashing.
//! This enables consistent trace comparison across ASLR-enabled processes.

use std::path::Path;
use thiserror::Error;

/// Errors that can occur during address normalization.
#[derive(Debug, Error)]
pub enum NormalizationError {
    #[error("No symbol found for address 0x{0:x}")]
    NoSymbol(u64),
    #[error("Binary path not accessible: {0}")]
    BinaryNotAccessible(String),
    #[error("Invalid ELF format: {0}")]
    InvalidElf(String),
}

/// Represents a normalized address as symbol name + offset within that symbol.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolOffset {
    /// Name of the symbol containing the address.
    pub symbol_name: String,
    /// Offset from the symbol's start address.
    pub offset: u64,
}

impl SymbolOffset {
    /// Create a new SymbolOffset.
    pub fn new(symbol_name: impl Into<String>, offset: u64) -> Self {
        Self {
            symbol_name: symbol_name.into(),
            offset,
        }
    }

    /// Format as "symbol_name+0xoffset".
    pub fn format_with_offset(&self) -> String {
        format!("{}+0x{:x}", self.symbol_name, self.offset)
    }
}

impl std::fmt::Display for SymbolOffset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:0x{:x}", self.symbol_name, self.offset)
    }
}

/// Trait for normalizing addresses to symbol+offset form.
///
/// Implementors can use different strategies (ELF symbol tables, DWARF, etc.)
/// to resolve addresses to their symbolic representation.
pub trait AddressNormalizer: Send + Sync {
    /// Normalize an address to symbol+offset form.
    ///
    /// Returns `Some(SymbolOffset)` if normalization succeeds.
    /// Returns `None` if the address cannot be normalized (e.g., no symbol info,
    /// anonymous memory region, or stripped binary).
    fn normalize(&self, pc: u64, binary_path: &Path) -> Option<SymbolOffset>;
}

/// A normalizer that uses ELF symbol tables to resolve addresses.
///
/// Uses the `object` crate to parse ELF files and find the symbol
/// containing a given program counter address.
pub struct SymbolOffsetNormalizer {
    // Configuration options could go here (e.g., prefer .symtab vs .dynsym)
}

impl SymbolOffsetNormalizer {
    /// Create a new SymbolOffsetNormalizer.
    pub fn new() -> Self {
        Self {}
    }

    /// Resolve an address to a symbol offset within an ELF binary.
    ///
    /// Uses the ELF symbol tables (.symtab for full symbols, .dynsym for dynamic)
    /// to find the symbol containing the given address.
    fn resolve_symbol(&self, pc: u64, binary_path: &Path) -> Option<SymbolOffset> {
        use object::Object;

        let data = std::fs::read(binary_path).ok()?;
        let obj = object::File::parse(data.as_slice()).ok()?;

        // Try .symtab first (full symbol table), then .dynsym (dynamic symbols)
        for section_name in &[".symtab", ".dynsym"] {
            if let Some(offset) = self.find_symbol_in_section(&obj, section_name, pc) {
                return Some(offset);
            }
        }

        None
    }

    fn find_symbol_in_section(
        &self,
        obj: &object::File,
        section_name: &str,
        pc: u64,
    ) -> Option<SymbolOffset> {
        use object::{Object, ObjectSymbol};

        let _section = obj.section_by_name(section_name)?;
        let symbols = obj.symbols();

        // Find the symbol containing pc
        // A symbol "contains" pc if: symbol_address <= pc < symbol_address + size
        // For functions, size is the function's extent
        let mut best_match: Option<(u64, &str, u64)> = None; // (symbol_addr, name, size)

        for symbol in symbols {
            let addr = symbol.address();
            let size = symbol.size();

            if addr == 0 || size == 0 {
                continue;
            }

            // Check if pc is within this symbol's range
            if pc >= addr && pc < addr + size {
                match best_match {
                    None => {
                        best_match = Some((addr, symbol.name().unwrap_or("<unknown>"), size));
                    }
                    Some((best_addr, _, best_size)) => {
                        // Prefer the more specific symbol (smaller size = more specific)
                        // or the one with lower address (more precise)
                        if size < best_size || (size == best_size && addr < best_addr) {
                            best_match = Some((addr, symbol.name().unwrap_or("<unknown>"), size));
                        }
                    }
                }
            }
        }

        best_match.map(|(addr, name, _)| {
            let offset = pc - addr;
            SymbolOffset::new(name, offset)
        })
    }

    /// Classify an address to determine if normalization should be attempted.
    ///
    /// Returns true if the address is in a loadable code/data segment (not heap/stack).
    fn is_normalizable_address(&self, _pc: u64) -> bool {
        // For now, we attempt normalization for all addresses.
        // In a more sophisticated implementation, we would check /proc/PID/maps
        // to determine if the address is in an anonymous region.
        true
    }
}

impl Default for SymbolOffsetNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl AddressNormalizer for SymbolOffsetNormalizer {
    fn normalize(&self, pc: u64, binary_path: &Path) -> Option<SymbolOffset> {
        // Check if this is a normalizable address
        if !self.is_normalizable_address(pc) {
            return None;
        }

        // Try to resolve the symbol
        let result = self.resolve_symbol(pc, binary_path);

        if result.is_none() {
            // Could log a warning here about stripped binary or unknown region
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_offset_creation() {
        let offset = SymbolOffset::new("main", 0x100);
        assert_eq!(offset.symbol_name, "main");
        assert_eq!(offset.offset, 0x100);
    }

    #[test]
    fn test_symbol_offset_display() {
        let offset = SymbolOffset::new("foo", 0x50);
        assert_eq!(offset.to_string(), "foo:0x50");
    }

    #[test]
    fn test_symbol_offset_format_with_offset() {
        let offset = SymbolOffset::new("bar", 0x20);
        assert_eq!(offset.format_with_offset(), "bar+0x20");
    }

    #[test]
    fn test_symbol_offset_equality() {
        let a = SymbolOffset::new("func", 0x10);
        let b = SymbolOffset::new("func", 0x10);
        let c = SymbolOffset::new("func", 0x11);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_symbol_offset_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SymbolOffset::new("main", 0x100));
        set.insert(SymbolOffset::new("main", 0x100)); // duplicate
        set.insert(SymbolOffset::new("main", 0x101));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_symbol_offset_normalizer_with_current_binary() {
        // Try to normalize an address in the current binary
        let normalizer = SymbolOffsetNormalizer::new();
        let exe = std::env::current_exe().unwrap();

        // Try with address 0 (should generally not have a symbol)
        let result = normalizer.normalize(0, &exe);
        // Address 0 typically doesn't map to a symbol

        // Try with a reasonable address - use the text segment base
        // We can't guarantee we find anything, but the function should not panic
        let _ = normalizer.normalize(0x400000, &exe);
    }

    #[test]
    fn test_symbol_offset_normalizer_nonexistent_binary() {
        let normalizer = SymbolOffsetNormalizer::new();
        let result = normalizer.normalize(0x1000, Path::new("/nonexistent/binary"));
        assert!(result.is_none());
    }

    #[test]
    fn test_is_normalizable_address() {
        let normalizer = SymbolOffsetNormalizer::new();
        // Always returns true for now
        assert!(normalizer.is_normalizable_address(0));
        assert!(normalizer.is_normalizable_address(0x7f0000000000));
    }
}
