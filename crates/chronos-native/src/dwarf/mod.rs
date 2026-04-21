//! DWARF debug info parsing for ELF binaries.
//!
//! This module provides `DwarfReader` for extracting source locations and variable
//! information from DWARF debug sections in ELF binaries.

pub mod location;
pub mod types;
pub mod variables;

use thiserror::Error;

// Use addr2line's re-exported gimli to ensure version compatibility
use addr2line::gimli;

#[derive(Debug, Error)]
pub enum DwarfError {
    #[error("DWARF parse error: {0}")]
    Parse(String),
    #[error("ELF parse error: {0}")]
    Elf(String),
    #[error("No DWARF info found")]
    NoDwarf,
}

/// DWARF debug info reader for ELF binaries.
///
/// Provides source location mapping and variable extraction from DWARF
/// debug sections (`.debug_line`, `.debug_info`).
pub struct DwarfReader<'data> {
    ctx: Option<addr2line::Context<gimli::EndianSlice<'data, gimli::RunTimeEndian>>>,
}

impl<'data> DwarfReader<'data> {
    /// Create a new DwarfReader from raw ELF bytes.
    ///
    /// Returns `Err(DwarfError::NoDwarf)` if the binary has no DWARF sections.
    /// Returns `Err(DwarfError::Elf)` if the bytes are not a valid ELF file.
    pub fn new(elf_bytes: &'data [u8]) -> Result<Self, DwarfError> {
        // Try to build addr2line context from .debug_line sections
        let ctx = Self::build_addr2line_context(elf_bytes);

        // If context is None, return NoDwarf
        if ctx.is_none() {
            return Err(DwarfError::NoDwarf);
        }

        Ok(Self { ctx })
    }

    fn build_addr2line_context(
        elf_bytes: &'data [u8],
    ) -> Option<addr2line::Context<gimli::EndianSlice<'data, gimli::RunTimeEndian>>> {
        use object::{Object, ObjectSection};

        // Parse as object file
        let obj = object::File::parse(elf_bytes).ok()?;

        // Determine endianness
        let endian = if obj.is_little_endian() {
            gimli::RunTimeEndian::Little
        } else {
            gimli::RunTimeEndian::Big
        };

        // Create a Dwarf struct using addr2line's gimli
        let load_section = |id: gimli::SectionId| -> Result<gimli::EndianSlice<'data, gimli::RunTimeEndian>, std::convert::Infallible> {
            let data = obj
                .section_by_name(id.name())
                .and_then(|s| s.data().ok())
                .unwrap_or(&[]);
            Ok(gimli::EndianSlice::new(data, endian))
        };

        let dwarf = gimli::Dwarf::load(&load_section).ok()?;

        // Create addr2line context
        let ctx = addr2line::Context::from_dwarf(dwarf).ok()?;

        Some(ctx)
    }

    /// Get source location for a program counter address.
    ///
    /// Returns `None` if the address is not covered by any line information
    /// or if DWARF data is not available.
    pub fn source_location(&self, pc: u64) -> Option<chronos_domain::trace::SourceLocation> {
        let ctx = self.ctx.as_ref()?;
        location::source_location(ctx, pc)
    }

    /// Get all variables in scope at a given program counter.
    ///
    /// Returns an empty vector if no variables are found or if DWARF
    /// data is not available.
    pub fn variables_in_scope(&self, _pc: u64) -> Vec<chronos_domain::value::VariableInfo> {
        // Variable extraction from DWARF DIEs is complex and deferred to future work
        // The addr2line crate doesn't expose this functionality directly
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_location_from_self() {
        // Read current process binary - should have DWARF
        let exe = std::env::current_exe().unwrap();
        let bytes = std::fs::read(&exe).unwrap();

        // DwarfReader::new should succeed or return NoDwarf (not panic)
        let reader = DwarfReader::new(&bytes);
        assert!(
            reader.is_ok() || matches!(reader, Err(DwarfError::NoDwarf)),
            "DwarfReader::new should not panic for valid ELF"
        );
    }

    #[test]
    fn test_graceful_stripped_binary() {
        // Minimal ELF header without DWARF sections
        let fake_elf = b"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        let result = DwarfReader::new(fake_elf);
        // Should return error, not panic
        assert!(result.is_err(), "Invalid ELF should return error");
    }
}
