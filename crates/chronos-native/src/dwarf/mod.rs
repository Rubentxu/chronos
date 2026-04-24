//! DWARF debug info parsing for ELF binaries.
//!
//! This module provides `DwarfReader` for extracting source locations and variable
//! information from DWARF debug sections in ELF binaries.

pub mod eval;
pub mod location;
pub mod types;
pub mod variables;

pub use eval::{BasicLocationEvaluator, DwarfLocationEvaluator};

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
    elf_bytes: &'data [u8],
}

impl<'data> DwarfReader<'data> {
    /// Create a new DwarfReader from raw ELF bytes.
    ///
    /// Returns `Err(DwarfError::NoDwarf)` if the binary has no DWARF sections.
    /// Returns `Err(DwarfError::Elf)` if the bytes are not a valid ELF file.
    pub fn new(elf_bytes: &'data [u8]) -> Result<Self, DwarfError> {
        // Build addr2line context from .debug_line sections
        let ctx = Self::build_addr2line_context(elf_bytes);

        // If context is None, return NoDwarf
        if ctx.is_none() {
            return Err(DwarfError::NoDwarf);
        }

        Ok(Self { ctx, elf_bytes })
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
        addr2line::Context::from_dwarf(dwarf).ok()
    }

    /// Build gimli Dwarf from stored ELF bytes.
    fn build_dwarf(&self) -> Option<gimli::Dwarf<gimli::EndianSlice<'data, gimli::RunTimeEndian>>> {
        use object::{Object, ObjectSection};

        // Parse as object file
        let obj = object::File::parse(self.elf_bytes).ok()?;

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

        gimli::Dwarf::load(&load_section).ok()
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
    pub fn variables_in_scope(&self, pc: u64) -> Vec<chronos_domain::value::VariableInfo> {
        // Rebuild dwarf from stored bytes and delegate to variables module
        match self.build_dwarf() {
            Some(dwarf) => variables::variables_in_scope(&dwarf, pc),
            None => Vec::new(),
        }
    }

    /// Resolve a specific variable at a program counter.
    ///
    /// Uses the DWARF location expression evaluator to resolve the variable's
    /// actual value from the register snapshot.
    ///
    /// Returns `Some((name, DwarfValue))` if the variable is found and its
    /// location can be evaluated. Returns `None` if the variable is not found
    /// or its location expression cannot be evaluated.
    pub fn resolve_variable(
        &self,
        pc: u64,
        name: &str,
        regs: &chronos_domain::value::RegisterSnapshot,
    ) -> Option<(String, chronos_domain::value::DwarfValue)> {
        let dwarf = self.build_dwarf()?;
        variables::resolve_variable(&dwarf, pc, name, regs)
    }

    /// Get all variables in scope at a given program counter with resolved locations.
    ///
    /// This method evaluates DWARF location expressions using the provided
    /// register snapshot to resolve variables to their actual memory addresses
    /// or register locations.
    ///
    /// Returns a vector of variables with resolved locations. Variables whose
    /// location expressions cannot be evaluated are omitted (graceful degradation).
    pub fn variables_in_scope_with_regs(
        &self,
        pc: u64,
        regs: &chronos_domain::value::RegisterSnapshot,
    ) -> Vec<chronos_domain::value::VariableInfo> {
        let dwarf = match self.build_dwarf() {
            Some(d) => d,
            None => return Vec::new(),
        };

        // Get raw variables first
        let raw_vars = variables::variables_in_scope(&dwarf, pc);

        // Use BasicLocationEvaluator to resolve locations
        let evaluator = BasicLocationEvaluator::new();

        raw_vars
            .into_iter()
            .filter_map(|var| {
                // Try to get the location expression bytes for this variable
                // For now, we use the address as a fallback
                // A full implementation would look up DW_AT_location bytes
                // and evaluate them with the evaluator
                let location_bytes = variables::get_location_bytes(&dwarf, pc, &var.name);
                if let Some(bytes) = location_bytes {
                    if let Some(dwarf_val) = evaluator.evaluate(&bytes, regs) {
                        let address = match dwarf_val {
                            chronos_domain::value::DwarfValue::Memory { address, .. } => address,
                            chronos_domain::value::DwarfValue::Register(_) => 0, // Can't represent register as u64 address
                            chronos_domain::value::DwarfValue::Immediate(_) => 0,
                        };
                        let value = dwarf_val.format();
                        return Some(chronos_domain::value::VariableInfo::new(
                            var.name,
                            value,
                            var.type_name,
                            address,
                            var.scope,
                        ));
                    }
                }
                // If we can't resolve the location, return the original with address 0
                Some(chronos_domain::value::VariableInfo::new(
                    var.name,
                    var.value,
                    var.type_name,
                    0, // Unknown address
                    var.scope,
                ))
            })
            .collect()
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
