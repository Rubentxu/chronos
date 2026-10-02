//! Source location within a program.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Location in source code corresponding to a trace event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default, JsonSchema)]
pub struct SourceLocation {
    /// Source file path (absolute or relative).
    pub file: Option<String>,
    /// Line number (1-based).
    pub line: Option<u32>,
    /// Column number (1-based).
    pub column: Option<u32>,
    /// Function name.
    pub function: Option<String>,
    /// Instruction address in memory.
    pub address: u64,
}

impl SourceLocation {
    /// Create a minimal location with just an address.
    pub fn from_address(address: u64) -> Self {
        Self {
            file: None,
            line: None,
            column: None,
            function: None,
            address,
        }
    }

    /// Create a full source location.
    pub fn new(
        file: impl Into<String>,
        line: u32,
        function: impl Into<String>,
        address: u64,
    ) -> Self {
        Self {
            file: Some(file.into()),
            line: Some(line),
            column: None,
            function: Some(function.into()),
            address,
        }
    }
}

impl std::fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.file, &self.function, &self.line) {
            (Some(file), Some(func), Some(line)) => {
                write!(f, "{}:{} ({} at 0x{:x})", file, line, func, self.address)
            }
            (Some(file), Some(func), None) => {
                write!(f, "{} ({} at 0x{:x})", file, func, self.address)
            }
            (Some(func), None, None) => {
                write!(f, "{} at 0x{:x}", func, self.address)
            }
            // Function known, source unknown. `SymbolInfo` resolves a name
            // from the ELF symbol table on every address but leaves `file`
            // and `line` as `None` ("None for MVP"), so this is the shape a
            // real resolved symbol produces. Without these arms the catch-all
            // below printed the bare address and dropped the function name
            // the resolver had actually found.
            (None, Some(func), line) => match line {
                Some(line) => write!(f, "{}:{} at 0x{:x}", func, line, self.address),
                None => write!(f, "{} at 0x{:x}", func, self.address),
            },
            _ => write!(f, "0x{:x}", self.address),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_address() {
        let loc = SourceLocation::from_address(0x401000);
        assert_eq!(loc.address, 0x401000);
        assert!(loc.file.is_none());
    }

    #[test]
    fn test_new_full() {
        let loc = SourceLocation::new("main.rs", 42, "main", 0x401000);
        assert_eq!(loc.file.as_deref(), Some("main.rs"));
        assert_eq!(loc.line, Some(42));
        assert_eq!(loc.function.as_deref(), Some("main"));
    }

    #[test]
    fn test_display_full() {
        let loc = SourceLocation::new("main.rs", 42, "main", 0x401000);
        let s = loc.to_string();
        assert!(s.contains("main.rs"));
        assert!(s.contains("42"));
        assert!(s.contains("main"));
    }

    #[test]
    fn test_display_address_only() {
        let loc = SourceLocation::from_address(0xDEAD);
        let s = loc.to_string();
        assert_eq!(s, "0xdead");
    }

    /// Discriminante. A resolved symbol carries a name but no source, and
    /// the catch-all arm used to print the bare address for that shape, so
    /// the one piece of information the resolver had found was dropped from
    /// the rendered location. Removing the `(None, Some(func), line)` arm
    /// fails this with `left: "0x401000"`.
    #[test]
    fn test_display_keeps_the_function_when_the_source_is_unknown() {
        let loc = SourceLocation {
            file: None,
            line: None,
            column: None,
            function: Some("main".into()),
            address: 0x401000,
        };
        assert_eq!(loc.to_string(), "main at 0x401000");

        let with_line = SourceLocation {
            line: Some(7),
            ..loc
        };
        assert_eq!(with_line.to_string(), "main:7 at 0x401000");
    }
}
