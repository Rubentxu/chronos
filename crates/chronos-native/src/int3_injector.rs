//! INT3 software-breakpoint injection for real function-frame capture.
//!
//! This module turns static ELF function-entry addresses into runtime
//! addresses the child will actually execute, then plants `0xCC` (INT3)
//! at those bytes via `PTRACE_POKEDATA`. When the tracee stops with a
//! SIGTRAP whose `RIP - 1` points at an installed breakpoint, the caller
//! is told that a function entry fired. The injector also knows the
//! original first byte of every instrumented function so the caller can
//! temporarily restore it, single-step over the real instruction, and
//! re-install the breakpoint for the next (possibly recursive) call.
//!
//! # Address relocation (ASLR)
//!
//! [`SymbolResolver`] reports *static* ELF addresses (e.g. `0x11a9` for a
//! PIE, `0x401185` for a `-no-pie` executable). The child runs with those
//! addresses shifted by the process load bias. For every mapped executable
//! this bias is `runtime_base_of_first_pt_load - first_pt_load_vaddr`; it
//! is `0` for `-no-pie` (fixed `0x400000`) and equals the randomized base
//! for PIE. `compute_load_bias` derives it from `/proc/<pid>/maps` plus the
//! ELF program headers, so the same code path handles both cases.

use crate::symbol_resolver::SymbolResolver;
use nix::sys::ptrace;
use nix::unistd::Pid;
use object::{Object, ObjectSegment};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use tracing::debug;

/// The x86_64 software-breakpoint opcode.
pub const INT3: u8 = 0xcc;

/// A breakpoint we planted at one relocated function entry.
#[derive(Debug, Clone)]
pub struct InstalledBreakpoint {
    /// Runtime address where `0xCC` currently lives (== symbol address + bias).
    pub address: u64,
    /// The original first byte of the function, restored before single-stepping.
    pub original_byte: u8,
    /// Function symbol name for this entry.
    pub symbol_name: String,
}

/// Manages a set of INT3 breakpoints for one traced process.
#[derive(Debug, Default)]
pub struct Int3Injector {
    /// All functions we decided to instrument, keyed by runtime address.
    by_address: HashMap<u64, InstalledBreakpoint>,
    /// Subset of `by_address` whose `0xCC` is currently in memory
    /// (an entry leaves this set while we single-step over its real byte).
    active: HashSet<u64>,
}

impl Int3Injector {
    /// Create an empty injector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of distinct instrumented function entries.
    pub fn len(&self) -> usize {
        self.by_address.len()
    }

    /// True when no function entries were instrumented.
    pub fn is_empty(&self) -> bool {
        self.by_address.is_empty()
    }

    /// Whether the INT3 at `address` is currently planted in memory.
    pub fn is_active(&self, address: u64) -> bool {
        self.active.contains(&address)
    }

    /// Compute the process load bias for a launched child.
    ///
    /// `bias` satisfies `runtime_function_address == static_symbol_address + bias`.
    /// It is derived from the `/proc/<pid>/maps` entry for the first `PT_LOAD`
    /// (file offset 0) minus that segment's `p_vaddr` read from the ELF headers.
    ///
    /// Returns `None` when the mapping or headers cannot be read (stripped,
    /// attached process with an anonymous image, etc.); the caller degrades by
    /// disabling frame capture rather than guessing.
    pub fn compute_load_bias(pid: i32, exe_path: &Path) -> Option<u64> {
        let maps = fs::read_to_string(format!("/proc/{}/maps", pid)).ok()?;
        let exe_real = fs::canonicalize(exe_path).ok()?;
        // The runtime start of the first PT_LOAD is the map row that maps the
        // executable's file offset 0.
        let mut runtime_base: Option<u64> = None;
        for line in maps.lines() {
            let mut it = line.split_whitespace();
            let range = it.next()?;
            let _perms = it.next()?;
            let offset = it.next()?;
            let _dev = it.next()?;
            let _inode = it.next()?;
            let path = it.collect::<Vec<_>>().join(" ");
            if path.is_empty() {
                continue;
            }
            // Match the mapping to our executable by canonicalized path OR by
            // inode-free prefix; the main-exe map row is the one whose path is
            // the launched binary.
            let path_matches = path == exe_real.to_string_lossy().as_ref()
                || path == exe_path.to_string_lossy().as_ref();
            if path_matches && offset == "00000000" {
                let start = u64::from_str_radix(range.split('-').next()?, 16).ok();
                if let Some(s) = start {
                    runtime_base = Some(s);
                    break;
                }
            }
        }
        let runtime_base = runtime_base?;

        let first_pt_load_vaddr = first_pt_load_vaddr(exe_path)?;
        Some(runtime_base.wrapping_sub(first_pt_load_vaddr))
    }

    /// Plant INT3 at the relocated entry of every size-bearing function
    /// symbol the resolver knows about. Returns the number installed.
    ///
    /// Reads each original first byte before overwriting it, so the caller
    /// can restore and single-step through the real instruction.
    pub fn install(
        &mut self,
        pid: Pid,
        resolver: &SymbolResolver,
        bias: u64,
    ) -> Result<usize, String> {
        let mut installed = 0usize;
        for sym in resolver.symbols().values() {
            if sym.address == 0 || sym.size == 0 {
                continue;
            }
            let runtime = sym.address.wrapping_add(bias);
            if runtime == 0 || self.by_address.contains_key(&runtime) {
                continue;
            }
            let original_byte = match read_byte(pid, runtime) {
                Ok(b) => b,
                Err(e) => {
                    debug!("int3: skip {} @0x{:x}: {}", sym.name, runtime, e);
                    continue;
                }
            };
            if original_byte == INT3 {
                // Already instrumented (duplicate address) — not our doing.
                continue;
            }
            if let Err(e) = write_byte(pid, runtime, INT3) {
                debug!(
                    "int3: failed to plant @0x{:x} ({}): {}",
                    runtime, sym.name, e
                );
                continue;
            }
            let bp = InstalledBreakpoint {
                address: runtime,
                original_byte,
                symbol_name: sym.name.clone(),
            };
            self.by_address.insert(runtime, bp);
            self.active.insert(runtime);
            installed += 1;
        }
        Ok(installed)
    }

    /// Given the `RIP` at a SIGTRAP stop, return the breakpoint that fired —
    /// the installed entry at `RIP - 1` — if any.
    pub fn hit_at_rip(&self, rip: u64) -> Option<&InstalledBreakpoint> {
        if rip == 0 {
            return None;
        }
        let addr = rip - 1;
        if self.active.contains(&addr) {
            self.by_address.get(&addr)
        } else {
            None
        }
    }

    /// Temporarily put back the function's real first byte (removing the
    /// breakpoint) so the caller can single-step past the entry instruction.
    pub fn restore_byte(&mut self, pid: Pid, address: u64) -> Result<(), String> {
        let bp = self
            .by_address
            .get(&address)
            .ok_or_else(|| format!("restore: no breakpoint recorded @0x{:x}", address))?;
        if self.active.remove(&address) {
            write_byte(pid, address, bp.original_byte)
        } else {
            Ok(())
        }
    }

    /// Re-plant the INT3 at `address` after its single-step completed.
    pub fn reinstall_byte(&mut self, pid: Pid, address: u64) -> Result<(), String> {
        if !self.by_address.contains_key(&address) {
            return Err(format!(
                "reinstall: no breakpoint recorded @0x{:x}",
                address
            ));
        }
        if !self.active.contains(&address) {
            write_byte(pid, address, INT3)?;
            self.active.insert(address);
        }
        Ok(())
    }

    /// Fully remove every still-active breakpoint (restore real bytes).
    /// Best-effort: used during shutdown so we do not leave INT3 in the
    /// process image for an attached/detached tracee.
    pub fn disarm(&mut self, pid: Pid) {
        let addrs: Vec<u64> = self.active.iter().copied().collect();
        for a in addrs {
            let _ = self.restore_byte(pid, a);
        }
    }
}

/// Read the `p_vaddr` of the first `PT_LOAD` segment from an ELF file.
///
/// The first loadable segment is the one whose file offset is 0. For a
/// `-no-pie` executable this is `0x400000`; for a PIE it is `0`.
fn first_pt_load_vaddr(path: &Path) -> Option<u64> {
    let bytes = fs::read(path).ok()?;
    let obj = object::File::parse(bytes.as_slice()).ok()?;
    let mut candidates: Vec<u64> = Vec::new();
    for seg in obj.segments() {
        let (file_off, _) = seg.file_range();
        if file_off == 0 {
            candidates.push(seg.address());
        }
    }
    candidates.into_iter().min()
}

/// Read the single byte at `address` using an aligned word `PTRACE_PEEKDATA`.
fn read_byte(pid: Pid, address: u64) -> Result<u8, String> {
    let aligned = address & !0x7u64;
    let shift = (address & 0x7) * 8;
    let word = ptrace::read(pid, aligned as ptrace::AddressType)
        .map_err(|e| format!("PEEKDATA @0x{:x}: {}", aligned, e))?;
    Ok(((word as u64) >> shift) as u8)
}

/// Write a single byte at `address` via an aligned word `PTRACE_POKEDATA`.
fn write_byte(pid: Pid, address: u64, byte: u8) -> Result<(), String> {
    let aligned = address & !0x7u64;
    let shift = (address & 0x7) * 8;
    let mask = 0xffu64 << shift;
    let word = ptrace::read(pid, aligned as ptrace::AddressType)
        .map_err(|e| format!("PEEKDATA @0x{:x}: {}", aligned, e))? as u64;
    let new_word = (word & !mask) | ((byte as u64) << shift);
    ptrace::write(
        pid,
        aligned as ptrace::AddressType,
        new_word as nix::libc::c_long,
    )
    .map_err(|e| format!("POKEDATA @0x{:x}: {}", aligned, e))
}
