//! Variable extraction from DWARF DIEs.
//!
//! This module provides functionality to extract local variables and function
//! parameters from DWARF debug information by traversing the DIE tree.

use chronos_domain::value::{VariableInfo, VariableScope};

// Use addr2line's re-exported gimli for type compatibility
use addr2line::gimli;
use gimli::read::{AttributeValue, DebuggingInformationEntry, Unit};

/// Get a string from a String attribute value.
///
/// `DW_AT_name` is encoded in two ways that matter in practice:
///
/// * `DW_FORM_string` — the bytes sit inline in `.debug_info`.
/// * `DW_FORM_strp` — an offset into `.debug_str`. This is what DWARF 4 (and
///   therefore gcc and clang by default) emits, so matching only the inline
///   form makes every name resolve to `None` on ordinary builds.
///
/// `Attribute::string_value` handles both, plus the DWARF 5 indexed forms, as
/// long as it is given the `.debug_str` section to read through. Returning
/// `None` here is not cosmetic: callers substitute `"unknown"`, so a missed
/// variant silently produces a variable list with no usable identity.
fn get_string_attr_value<R: gimli::Reader>(
    attr: &gimli::Attribute<R>,
    debug_str: &gimli::DebugStr<R>,
) -> Option<String> {
    // `string_value` hands back the reader parked on the string, and the
    // `Cow` it yields may borrow from that local reader. Copying to an owned
    // `String` *inside* the closure keeps that borrow from escaping it.
    attr.string_value(debug_str)
        .and_then(|reader| reader.to_string().ok().map(|cow| cow.to_string()))
}

/// Get a type reference from an attribute value.
fn get_type_ref<R: gimli::Reader<Offset = usize>>(
    attr: &gimli::Attribute<R>,
) -> Option<gimli::read::UnitOffset> {
    match attr.value() {
        AttributeValue::UnitRef(type_ref) => Some(type_ref),
        _ => None,
    }
}

/// Get the name of a type from a type reference offset.
fn resolve_type_name<R: gimli::Reader<Offset = usize>>(
    unit: &Unit<R>,
    type_offset: gimli::read::UnitOffset,
    debug_str: &gimli::DebugStr<R>,
) -> String {
    // Use entries_at_offset to get the type DIE directly
    let mut cursor = match unit.entries_at_offset(type_offset) {
        Ok(c) => c,
        Err(_) => return "unknown".to_string(),
    };

    // First entry is the type itself
    if let Ok(Some((_, entry))) = cursor.next_dfs() {
        // Get DW_AT_name from this type DIE
        if let Ok(Some(attr)) = entry.attr(gimli::DW_AT_name) {
            if let Some(name) = get_string_attr_value(&attr, debug_str) {
                return name;
            }
        }
    }

    "unknown".to_string()
}

/// Check if a PC is within a function's address range.
///
/// `DW_AT_high_pc` has two mutually exclusive encodings, and reading it
/// correctly is the whole job here:
///
/// * **DWARF 5** encodes it as an address (`DW_FORM_addr`). The value is
///   already absolute, so the range is `[low_pc, high_pc)`.
/// * **DWARF <= 4** encodes it as a *length relative to* `low_pc`
///   (`DW_FORM_data1/2/4/8`). gcc and clang emit this by default, so it is
///   the common case, not the exotic one. gimli normalises those forms to
///   `AttributeValue::Udata`.
///
/// The two are indistinguishable by value: an absolute high address and a
/// function length are both small unsigned integers. Only the tag says which
/// one it is, so the variant must be matched rather than the number
/// reinterpreted. Treating a length as an absolute address silently rejects
/// every function, because `pc < 160` is almost never true.
fn is_pc_in_function<R: gimli::Reader<Offset = usize>>(
    _unit: &Unit<R>,
    entry: &DebuggingInformationEntry<R>,
    pc: u64,
) -> bool {
    let Ok(Some(low_attr)) = entry.attr(gimli::DW_AT_low_pc) else {
        return false;
    };
    let AttributeValue::Addr(low) = low_attr.value() else {
        return false;
    };
    let Ok(Some(high_attr)) = entry.attr(gimli::DW_AT_high_pc) else {
        return false;
    };

    match high_attr.value() {
        // DWARF 5: an absolute address.
        AttributeValue::Addr(high) => pc >= low && pc < high,
        // DWARF <= 4: a length relative to low_pc. `Data*` are matched
        // alongside `Udata` so a reader that stops normalising them keeps
        // working; every spelling carries the same length.
        AttributeValue::Udata(len) => in_range(pc, low, len),
        AttributeValue::Data1(len) => in_range(pc, low, u64::from(len)),
        AttributeValue::Data2(len) => in_range(pc, low, u64::from(len)),
        AttributeValue::Data4(len) => in_range(pc, low, u64::from(len)),
        AttributeValue::Data8(len) => in_range(pc, low, len),
        // Anything else (a section offset, an expression, a flag) carries no
        // range this function can interpret.
        _ => false,
    }
}

/// Half-open range test against a function whose upper bound is expressed as
/// a length from its lower bound.
///
/// `low + len` is computed with `checked_add` so a corrupt or absurd length
/// cannot wrap into a range that accidentally contains `pc`.
fn in_range(pc: u64, low: u64, len: u64) -> bool {
    match low.checked_add(len) {
        Some(end) => pc >= low && pc < end,
        None => false,
    }
}

/// Find the subprogram DIE containing the given PC and extract its variables.
fn find_variables_in_cu<R: gimli::Reader<Offset = usize>>(
    unit: &Unit<R>,
    pc: u64,
    debug_str: &gimli::DebugStr<R>,
) -> Vec<VariableInfo> {
    let mut variables = Vec::new();

    // Use the unit's entries iterator with DFS to find the subprogram
    let mut cursor = unit.entries();
    let mut depth = 0isize;
    let mut found_function_depth = None;

    while let Ok(Some((delta_depth, entry))) = cursor.next_dfs() {
        depth += delta_depth;

        // Check if this is a subprogram
        if entry.tag() == gimli::DW_TAG_subprogram {
            // Check if PC is in this function
            if is_pc_in_function(unit, entry, pc) {
                // We found the function - record the depth and start collecting variables
                found_function_depth = Some(depth);
            } else if let Some(found_depth) = found_function_depth {
                // We've finished the function when we see another subprogram at same or higher depth
                if depth <= found_depth {
                    break;
                }
            }
        }

        // If we're inside the function, collect variables and parameters
        if found_function_depth.is_some() {
            let tag = entry.tag();
            let is_variable = tag == gimli::DW_TAG_variable;
            let is_parameter = tag == gimli::DW_TAG_formal_parameter;

            if is_variable || is_parameter {
                let name = entry
                    .attr(gimli::DW_AT_name)
                    .ok()
                    .flatten()
                    .and_then(|attr| get_string_attr_value(&attr, debug_str))
                    .unwrap_or_else(|| "unknown".to_string());

                let type_name = entry
                    .attr(gimli::DW_AT_type)
                    .ok()
                    .flatten()
                    .and_then(|attr| get_type_ref(&attr))
                    .map(|type_ref| resolve_type_name(unit, type_ref, debug_str))
                    .unwrap_or_else(|| "unknown".to_string());

                let address = 0u64;
                let scope = if is_parameter {
                    VariableScope::Parameter
                } else {
                    VariableScope::Local
                };

                variables.push(VariableInfo::new(
                    name,
                    format!("0x{:x}", address),
                    type_name,
                    address,
                    scope,
                ));
            }
        }
    }

    variables
}

/// Get all variables in scope at a given program counter address.
///
/// Traverses the DWARF debug info to find the function containing the PC,
/// then extracts all local variables and parameters from that function.
pub fn variables_in_scope<R: gimli::Reader<Offset = usize>>(
    dwarf: &gimli::Dwarf<R>,
    pc: u64,
) -> Vec<VariableInfo> {
    // Iterate through compilation units
    let mut units = dwarf.debug_info.units();

    while let Ok(Some(header)) = units.next() {
        // Parse the compilation unit
        if let Ok(unit) = dwarf.unit(header) {
            // Try to find the function containing this PC in this CU
            let vars = find_variables_in_cu(&unit, pc, &dwarf.debug_str);
            if !vars.is_empty() {
                return vars;
            }
        }
    }

    Vec::new()
}

/// Get the location expression bytes for a variable at a given PC.
///
/// Returns `Some(bytes)` if the variable has a DW_AT_location attribute,
/// `None` otherwise.
pub fn get_location_bytes<R: gimli::Reader<Offset = usize>>(
    dwarf: &gimli::Dwarf<R>,
    pc: u64,
    var_name: &str,
) -> Option<Vec<u8>> {
    let mut units = dwarf.debug_info.units();

    while let Ok(Some(header)) = units.next() {
        if let Ok(unit) = dwarf.unit(header) {
            if let Some(bytes) = find_location_bytes_in_cu(&unit, pc, var_name, &dwarf.debug_str) {
                return Some(bytes);
            }
        }
    }

    None
}

/// Find the location bytes for a variable in a compilation unit.
#[allow(dead_code)]
fn find_location_bytes_in_cu<R: gimli::Reader<Offset = usize>>(
    unit: &Unit<R>,
    pc: u64,
    var_name: &str,
    debug_str: &gimli::DebugStr<R>,
) -> Option<Vec<u8>> {
    let mut cursor = unit.entries();
    let mut depth = 0isize;
    let mut found_function_depth = None;

    while let Ok(Some((delta_depth, entry))) = cursor.next_dfs() {
        depth += delta_depth;

        // Check if this is a subprogram
        if entry.tag() == gimli::DW_TAG_subprogram {
            if is_pc_in_function(unit, entry, pc) {
                found_function_depth = Some(depth);
            } else if let Some(found_depth) = found_function_depth {
                if depth <= found_depth {
                    break;
                }
            }
        }

        // If we're inside the function, look for the variable
        if found_function_depth.is_some() {
            let tag = entry.tag();
            let is_variable =
                tag == gimli::DW_TAG_variable || tag == gimli::DW_TAG_formal_parameter;

            if is_variable {
                // Check if this is our variable
                let name = entry
                    .attr(gimli::DW_AT_name)
                    .ok()
                    .flatten()
                    .and_then(|attr| get_string_attr_value(&attr, debug_str));

                if name.as_deref() == Some(var_name) {
                    // Found the variable - get location
                    if let Ok(Some(attr)) = entry.attr(gimli::DW_AT_location) {
                        // For now, we can't easily extract location expression bytes
                        // This is a limitation of the current implementation
                        let _ = attr;
                    }
                }
            }
        }
    }

    None
}

/// Resolve a named variable at a given PC using register snapshot.
///
/// Returns `Some((name, DwarfValue))` if the variable is found and
/// its location can be evaluated.
pub fn resolve_variable<R: gimli::Reader<Offset = usize>>(
    dwarf: &gimli::Dwarf<R>,
    pc: u64,
    name: &str,
    regs: &chronos_domain::value::RegisterSnapshot,
) -> Option<(String, chronos_domain::value::DwarfValue)> {
    use crate::dwarf::BasicLocationEvaluator;
    use crate::dwarf::DwarfLocationEvaluator;

    // Find the location bytes for this variable
    let loc_bytes = get_location_bytes(dwarf, pc, name)?;

    // Evaluate using BasicLocationEvaluator
    let evaluator = BasicLocationEvaluator::new();
    let dwarf_val = evaluator.evaluate(&loc_bytes, regs)?;

    Some((name.to_string(), dwarf_val))
}

#[cfg(test)]
mod tests {
    use super::super::DwarfReader;

    /// A `DwarfReader` over the committed C fixture, shared per process.
    ///
    /// The fixture is 18 KB, so parsing it is cheap, but the reader is still
    /// built once: the tests below scan an address range, and each scan
    /// re-walks every compilation unit. `DwarfReader` is not `Sync` — gimli
    /// keeps line caches in an `UnsafeCell` — so the shared reader lives
    /// behind a `Mutex`, which also keeps the concurrent `variables_in_scope`
    /// calls that would otherwise be unsound.
    ///
    /// This is deliberately strict. The fixture is committed to the tree, so a
    /// reader that cannot be built means the premise of these tests is gone and
    /// they should fail loudly rather than skip into a green.
    fn with_fixture<T>(f: impl FnOnce(&DwarfReader<'static>) -> T) -> T {
        static READER: std::sync::OnceLock<std::sync::Mutex<DwarfReader<'static>>> =
            std::sync::OnceLock::new();
        let guard = READER
            .get_or_init(|| {
                let path = concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/test_dwarf_vars"
                );
                let raw = std::fs::read(path).expect("the DWARF fixture is committed and readable");
                let bytes: &'static [u8] = Box::leak(raw.into_boxed_slice());
                std::sync::Mutex::new(
                    DwarfReader::new(bytes).expect("the fixture carries DWARF variable info"),
                )
            })
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&guard)
    }

    /// Find the address inside the fixture whose enclosing function exposes the
    /// most variables, so assertions do not hard-code addresses that would move
    /// whenever the fixture is rebuilt.
    fn busiest_frame(
        reader: &DwarfReader<'static>,
    ) -> (u64, Vec<chronos_domain::value::VariableInfo>) {
        let mut best: Option<(u64, Vec<chronos_domain::value::VariableInfo>)> = None;
        for pc in (0..0x10_000u64).step_by(0x10) {
            let vars = reader.variables_in_scope(pc);
            if vars.is_empty() {
                continue;
            }
            if best.as_ref().is_none_or(|(_, b)| vars.len() > b.len()) {
                best = Some((pc, vars));
            }
        }
        best.expect("the fixture defines functions with local variables")
    }

    /// The function that holds the PC must yield its own named locals.
    ///
    /// This test used to end in `assert!(result.is_empty() || !result.is_empty())`,
    /// which is true for every possible result. It looked like coverage and was
    /// not: `variables_in_scope` was returning an empty vector for every address,
    /// for every build, because `is_pc_in_function` only understood
    /// `DW_AT_high_pc` in its DWARF 5 absolute form while gcc emits the DWARF 4
    /// length-relative form. The names assertion is what discriminates here —
    /// an implementation returning `[]` always fails it.
    #[test]
    fn variables_in_scope_returns_the_named_locals_of_the_enclosing_function() {
        with_fixture(|reader| {
            let (pc, vars) = busiest_frame(reader);
            let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();

            // `simple_function(int param1, int param2)` in
            // `tests/fixtures/test_dwarf_vars.c` declares exactly these.
            for expected in [
                "param1",
                "param2",
                "local_sum",
                "local_product",
                "local_diff",
                "local_char",
                "local_double",
                "local_ptr",
            ] {
                assert!(
                    names.contains(&expected),
                    "pc {pc:#x} is inside simple_function, so {expected} must be in scope; got {names:?}"
                );
            }
        });
    }

    /// Names are only useful if they are the real ones.
    ///
    /// `get_string_attr_value` used to accept only `DW_FORM_string`, the inline
    /// form. Real toolchains emit `DW_FORM_strp`, an offset into `.debug_str`,
    /// so every name silently degraded to the literal `"unknown"`. This
    /// separates that failure from the range failure above: it can only pass if
    /// the `.debug_str` lookup is wired through.
    #[test]
    fn variable_names_are_resolved_from_debug_str_not_placeholder_text() {
        with_fixture(|reader| {
            let (pc, vars) = busiest_frame(reader);
            let unknown = vars.iter().filter(|v| v.name == "unknown").count();
            assert_eq!(
                unknown,
                0,
                "pc {pc:#x} variables must carry their real names, not the {unknown}-variable \
                 fallback: got {:?}",
                vars.iter().map(|v| &v.name).collect::<Vec<_>>()
            );
        });
    }

    /// A different function must yield its own variables, not the neighbour's.
    ///
    /// `no_params_function` declares `standalone` and `another` and nothing
    /// else, so it doubles as the boundary check: the half-open range built
    /// from `low_pc + length` has to change the answer the moment the PC
    /// crosses into it.
    #[test]
    fn a_second_function_yields_its_own_variables() {
        with_fixture(|reader| {
            let second = (0..0x10_000u64)
                .step_by(0x10)
                .filter_map(|pc| {
                    let vars = reader.variables_in_scope(pc);
                    (!vars.is_empty()).then_some((pc, vars))
                })
                .find(|(_, vars)| {
                    let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
                    names.contains(&"standalone")
                });

            let (pc, second_vars) = second.expect(
                "the fixture also defines no_params_function, whose locals must be reachable",
            );
            let names: Vec<&str> = second_vars.iter().map(|v| v.name.as_str()).collect();
            assert_eq!(
                names,
                vec!["standalone", "another"],
                "no_params_function declares exactly these two, and nothing else, at pc {pc:#x}"
            );
            assert_ne!(
                pc, 0,
                "the second function must live at a real address, not fall through"
            );
        });
    }

    /// Graceful degradation: a PC that belongs to no function yields nothing
    /// rather than panicking or inventing variables.
    ///
    /// This test previously had a body of nothing but comments while its name
    /// promised exactly this.
    #[test]
    fn variables_in_scope_is_empty_for_a_pc_outside_every_function() {
        with_fixture(|reader| {
            for pc in [0u64, 0xdead_beef, 0x7fff_ffff_ffff] {
                assert!(
                    reader.variables_in_scope(pc).is_empty(),
                    "pc {pc:#x} is in no function, so it must yield no variables"
                );
            }
        });
    }

    /// A truncated ELF header is rejected outright rather than parsed into
    /// nonsense.
    ///
    /// This is the negative-input half of the reader's contract and it already
    /// held; it stays.
    #[test]
    fn a_truncated_elf_header_is_rejected_rather_than_parsed() {
        let truncated = b"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        assert!(
            DwarfReader::new(truncated).is_err(),
            "14 bytes cannot be a usable ELF image, so the reader must refuse it"
        );
    }
}
