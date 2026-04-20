//! Symbol resolution from ELF/DWARF debug information.
//!
//! Provides `SymbolResolver` that loads symbol tables from native binaries
//! and resolves instruction addresses to function names and source locations.
//!
//! MVP: Uses ELF symbol table (`.symtab` / `.dynsym`) for address → name mapping.
//! Future: Full DWARF `.debug_line` for file/line resolution.

use chronos_domain::trace::SourceLocation;
use object::{Object, ObjectSymbol};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use tracing::info;

/// Information about a resolved symbol.
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    /// Symbol name (function or label).
    pub name: String,
    /// Start address of the symbol.
    pub address: u64,
    /// Size of the symbol in bytes.
    pub size: u64,
    /// Source file (if available from DWARF, None for MVP).
    pub file: Option<String>,
    /// Source line number (if available from DWARF, None for MVP).
    pub line: Option<u32>,
}

impl SymbolInfo {
    /// Check if an address falls within this symbol's range.
    pub fn contains_address(&self, addr: u64) -> bool {
        if self.size == 0 {
            // Zero-size symbols: match if address is exactly at start
            addr == self.address
        } else {
            addr >= self.address && addr < (self.address + self.size)
        }
    }
}

/// Resolves instruction addresses to symbol information.
///
/// Loads the symbol table from an ELF binary and provides fast address lookup.
/// Uses a BTreeMap for efficient range queries.
pub struct SymbolResolver {
    /// Sorted map: address → SymbolInfo.
    symbols: BTreeMap<u64, SymbolInfo>,
    /// Path to the binary that was loaded.
    binary_path: String,
}

impl SymbolResolver {
    /// Create a new empty resolver.
    pub fn new() -> Self {
        Self {
            symbols: BTreeMap::new(),
            binary_path: String::new(),
        }
    }

    /// Load symbols from a binary file.
    ///
    /// Parses the ELF symbol table (`.symtab` and `.dynsym`) and indexes
    /// all function symbols by their start address.
    pub fn load_from_binary(&mut self, path: &Path) -> Result<(), String> {
        let file = File::open(path)
            .map_err(|e| format!("Failed to open binary '{}': {}", path.display(), e))?;

        let mmap = unsafe {
            memmap2::Mmap::map(&file)
                .map_err(|e| format!("Failed to mmap '{}': {}", path.display(), e))?
        };

        let object_file = object::File::parse(&*mmap)
            .map_err(|e| format!("Failed to parse '{}': {}", path.display(), e))?;

        self.binary_path = path.to_string_lossy().to_string();
        self.symbols.clear();

        let mut count = 0usize;

        // Load symbols from the main symbol table
        for symbol in object_file.symbols() {
            if let Some(info) = Self::extract_symbol_info(&symbol) {
                self.symbols.insert(info.address, info);
                count += 1;
            }
        }

        // Also load dynamic symbols (for shared libraries)
        for symbol in object_file.dynamic_symbols() {
            if let Some(info) = Self::extract_symbol_info(&symbol) {
                // Don't overwrite static symbols with dynamic ones
                self.symbols.entry(info.address).or_insert(info);
                count += 1;
            }
        }

        info!(
            "Loaded {} symbols from '{}'",
            count,
            path.display()
        );

        Ok(())
    }

    /// Extract symbol info from an object symbol, if it's a function.
    fn extract_symbol_info(
        symbol: &object::Symbol<'_, '_>,
    ) -> Option<SymbolInfo> {
        // Only care about functions (Text section symbols)
        let kind = symbol.kind();
        if !matches!(
            kind,
            object::SymbolKind::Text | object::SymbolKind::Label
        ) {
            return None;
        }

        let name = symbol.name().ok()?.to_string();
        if name.is_empty() || name.starts_with('.') {
            return None;
        }

        let address = symbol.address();
        let size = symbol.size();

        Some(SymbolInfo {
            name,
            address,
            size,
            file: None,   // MVP: no DWARF file info
            line: None,    // MVP: no DWARF line info
        })
    }

    /// Resolve an address to a symbol.
    ///
    /// First tries an exact match, then searches for the nearest symbol
    /// whose range contains the address.
    pub fn resolve(&self, address: u64) -> Option<&SymbolInfo> {
        // Try exact match first
        if let Some(sym) = self.symbols.get(&address) {
            return Some(sym);
        }

        // Find the symbol whose range contains this address.
        // Look at the symbol just before this address.
        let (_, sym) = self.symbols.range(..=address).next_back()?;

        if sym.contains_address(address) {
            Some(sym)
        } else {
            None
        }
    }

    /// Resolve an address to a SourceLocation.
    ///
    /// Returns a SourceLocation with the function name and address,
    /// or a minimal location with just the address if not resolved.
    pub fn resolve_to_source_location(&self, address: u64) -> SourceLocation {
        match self.resolve(address) {
            Some(sym) => SourceLocation {
                file: sym.file.clone(),
                line: sym.line,
                column: None,
                function: Some(sym.name.clone()),
                address,
            },
            None => SourceLocation::from_address(address),
        }
    }

    /// Get all loaded symbols.
    pub fn symbols(&self) -> &BTreeMap<u64, SymbolInfo> {
        &self.symbols
    }

    /// Get the number of loaded symbols.
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    /// Get the binary path that was loaded.
    pub fn binary_path(&self) -> &str {
        &self.binary_path
    }

    /// Check if any symbols are loaded.
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Find all symbols whose name matches a pattern.
    ///
    /// Pattern supports `*` (any chars) and `?` (single char).
    pub fn find_by_name(&self, pattern: &str) -> Vec<&SymbolInfo> {
        self.symbols
            .values()
            .filter(|sym| simple_glob_match(&sym.name, pattern))
            .collect()
    }

    /// Get function entry addresses for all symbols matching a pattern.
    ///
    /// Useful for setting breakpoints by function name.
    pub fn get_function_addresses(&self, pattern: &str) -> Vec<(String, u64)> {
        self.find_by_name(pattern)
            .into_iter()
            .map(|sym| (sym.name.clone(), sym.address))
            .collect()
    }
}

impl Default for SymbolResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple glob matching: supports `*` (any chars) and `?` (single char).
/// Copied from chronos-domain to keep this crate self-contained.
fn simple_glob_match(text: &str, pattern: &str) -> bool {
    let text_chars: Vec<char> = text.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();
    glob_match_inner(&text_chars, &pattern_chars, 0, 0)
}

fn glob_match_inner(text: &[char], pattern: &[char], ti: usize, pi: usize) -> bool {
    if pi == pattern.len() {
        return ti == text.len();
    }

    if pattern[pi] == '*' {
        for i in ti..=text.len() {
            if glob_match_inner(text, pattern, i, pi + 1) {
                return true;
            }
        }
        false
    } else if ti < text.len()
        && (pattern[pi] == '?' || pattern[pi] == text[ti])
    {
        glob_match_inner(text, pattern, ti + 1, pi + 1)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_info_contains_address() {
        let sym = SymbolInfo {
            name: "main".into(),
            address: 0x1000,
            size: 100,
            file: None,
            line: None,
        };

        assert!(sym.contains_address(0x1000));
        assert!(sym.contains_address(0x1050));
        assert!(sym.contains_address(0x1063)); // 0x1000 + 99
        assert!(!sym.contains_address(0x0FFF));
        assert!(!sym.contains_address(0x1064)); // 0x1000 + 100 = exclusive end
    }

    #[test]
    fn test_symbol_info_zero_size() {
        let sym = SymbolInfo {
            name: "label".into(),
            address: 0x2000,
            size: 0,
            file: None,
            line: None,
        };

        assert!(sym.contains_address(0x2000));
        assert!(!sym.contains_address(0x2001));
    }

    #[test]
    fn test_resolver_new() {
        let resolver = SymbolResolver::new();
        assert!(resolver.is_empty());
        assert_eq!(resolver.symbol_count(), 0);
    }

    #[test]
    fn test_resolver_default() {
        let resolver = SymbolResolver::default();
        assert!(resolver.is_empty());
    }

    #[test]
    fn test_resolver_load_invalid_path() {
        let mut resolver = SymbolResolver::new();
        let result = resolver.load_from_binary(Path::new("/nonexistent/binary"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to open"));
    }

    #[test]
    fn test_resolver_load_not_elf() {
        let mut resolver = SymbolResolver::new();
        // /dev/null is not an ELF file
        let result = resolver.load_from_binary(Path::new("/dev/null"));
        assert!(result.is_err());
    }

    #[test]
    fn test_resolver_load_real_binary() {
        let mut resolver = SymbolResolver::new();
        // /bin/ls should exist and be an ELF binary with symbols
        let result = resolver.load_from_binary(Path::new("/bin/ls"));
        if result.is_ok() {
            assert!(!resolver.is_empty());
            assert!(resolver.symbol_count() > 0);
            assert!(resolver.binary_path().contains("ls"));
        }
        // If it fails (stripped binary), that's also acceptable
    }

    #[test]
    fn test_resolver_resolve_empty() {
        let resolver = SymbolResolver::new();
        assert!(resolver.resolve(0x1000).is_none());
    }

    #[test]
    fn test_resolver_manual_symbols() {
        let mut resolver = SymbolResolver::new();

        // Manually insert symbols
        resolver.symbols.insert(0x1000, SymbolInfo {
            name: "main".into(),
            address: 0x1000,
            size: 50,
            file: None,
            line: None,
        });
        resolver.symbols.insert(0x2000, SymbolInfo {
            name: "helper".into(),
            address: 0x2000,
            size: 100,
            file: None,
            line: None,
        });

        // Exact match
        let sym = resolver.resolve(0x1000).unwrap();
        assert_eq!(sym.name, "main");

        // Within range
        let sym = resolver.resolve(0x2030).unwrap();
        assert_eq!(sym.name, "helper");

        // Before first symbol
        assert!(resolver.resolve(0x0500).is_none());

        // Between symbols but not in range
        assert!(resolver.resolve(0x1050).is_none()); // main ends at 0x1032
    }

    #[test]
    fn test_resolve_to_source_location() {
        let mut resolver = SymbolResolver::new();
        resolver.symbols.insert(0x1000, SymbolInfo {
            name: "process_data".into(),
            address: 0x1000,
            size: 200,
            file: Some("main.rs".into()),
            line: Some(42),
        });

        let loc = resolver.resolve_to_source_location(0x1050);
        assert_eq!(loc.function.as_deref(), Some("process_data"));
        assert_eq!(loc.file.as_deref(), Some("main.rs"));
        assert_eq!(loc.line, Some(42));
        assert_eq!(loc.address, 0x1050);
    }

    #[test]
    fn test_resolve_to_source_location_unknown() {
        let resolver = SymbolResolver::new();
        let loc = resolver.resolve_to_source_location(0xDEAD);
        assert!(loc.function.is_none());
        assert_eq!(loc.address, 0xDEAD);
    }

    #[test]
    fn test_find_by_name() {
        let mut resolver = SymbolResolver::new();
        resolver.symbols.insert(0x1000, SymbolInfo {
            name: "main".into(),
            address: 0x1000,
            size: 50,
            file: None,
            line: None,
        });
        resolver.symbols.insert(0x2000, SymbolInfo {
            name: "helper_one".into(),
            address: 0x2000,
            size: 50,
            file: None,
            line: None,
        });
        resolver.symbols.insert(0x3000, SymbolInfo {
            name: "helper_two".into(),
            address: 0x3000,
            size: 50,
            file: None,
            line: None,
        });

        // Exact match
        let results = resolver.find_by_name("main");
        assert_eq!(results.len(), 1);

        // Glob pattern
        let results = resolver.find_by_name("helper_*");
        assert_eq!(results.len(), 2);

        // Wildcard
        let results = resolver.find_by_name("*");
        assert_eq!(results.len(), 3);

        // No match
        let results = resolver.find_by_name("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_get_function_addresses() {
        let mut resolver = SymbolResolver::new();
        resolver.symbols.insert(0x1000, SymbolInfo {
            name: "main".into(),
            address: 0x1000,
            size: 50,
            file: None,
            line: None,
        });
        resolver.symbols.insert(0x2000, SymbolInfo {
            name: "my_func".into(),
            address: 0x2000,
            size: 100,
            file: None,
            line: None,
        });

        let addrs = resolver.get_function_addresses("my_*");
        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0], ("my_func".to_string(), 0x2000));
    }

    #[test]
    fn test_glob_match() {
        assert!(simple_glob_match("main", "main"));
        assert!(simple_glob_match("main", "mai*"));
        assert!(simple_glob_match("helper_one", "helper_*"));
        assert!(simple_glob_match("a", "?"));
        assert!(!simple_glob_match("ab", "?"));
        assert!(simple_glob_match("anything", "*"));
        assert!(!simple_glob_match("main", "helper"));
    }
}
