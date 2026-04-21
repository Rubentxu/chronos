//! Source location within a program.

use serde::{Deserialize, Serialize};

/// Location in source code corresponding to a trace event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
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
}
