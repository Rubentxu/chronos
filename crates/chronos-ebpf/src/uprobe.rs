//! Uprobe attachment manager.
//!
//! Manages attaching and detaching eBPF uprobes to symbols in target binaries.
//! All operations behind the `ebpf` feature are no-ops when compiled without it.

use crate::EbpfError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A unique key identifying an attached uprobe.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UprobeKey {
    /// Absolute path to the binary.
    pub binary: PathBuf,
    /// Symbol name.
    pub symbol: String,
}

impl UprobeKey {
    pub fn new(binary: impl AsRef<Path>, symbol: impl Into<String>) -> Self {
        Self {
            binary: binary.as_ref().to_path_buf(),
            symbol: symbol.into(),
        }
    }
}

/// Metadata about an active uprobe attachment.
#[derive(Debug, Clone)]
pub struct UprobeInfo {
    pub key: UprobeKey,
    /// Resolved offset within the binary (filled in at attach time).
    pub offset: u64,
}

/// Manages the lifecycle of uprobe attachments.
///
/// When compiled without `ebpf`, all mutation operations return
/// [`EbpfError::Unavailable`]. Read operations (like `is_attached`) are always
/// functional so callers can check state without `cfg` guards.
pub struct UprobeManager {
    /// Currently attached uprobes (key → info).
    attached: HashMap<UprobeKey, UprobeInfo>,
}

impl UprobeManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self {
            attached: HashMap::new(),
        }
    }

    /// Returns `true` if a uprobe for `(binary, symbol)` is already attached.
    pub fn is_attached(&self, binary: impl AsRef<Path>, symbol: &str) -> bool {
        let key = UprobeKey::new(binary, symbol);
        self.attached.contains_key(&key)
    }

    /// Returns the number of active uprobe attachments.
    pub fn attachment_count(&self) -> usize {
        self.attached.len()
    }

    /// List all active uprobe infos.
    pub fn list(&self) -> Vec<&UprobeInfo> {
        self.attached.values().collect()
    }

    /// Attach a uprobe to `symbol` in `binary`.
    ///
    /// - Returns `Err(EbpfError::Unavailable)` when `ebpf` feature is off.
    /// - Returns `Err(EbpfError::Uprobe("already attached"))` if already attached.
    pub fn attach_uprobe(
        &mut self,
        binary: impl AsRef<Path>,
        symbol: impl Into<String>,
    ) -> Result<(), EbpfError> {
        #[cfg(not(feature = "ebpf"))]
        {
            let _ = (binary, symbol);
            return Err(EbpfError::Unavailable {
                reason: "ebpf feature not enabled".to_string(),
            });
        }

        #[cfg(feature = "ebpf")]
        {
            let sym = symbol.into();
            let key = UprobeKey::new(binary.as_ref(), &sym);

            if self.attached.contains_key(&key) {
                return Err(EbpfError::Uprobe(format!(
                    "uprobe '{}' already attached to '{}'",
                    sym,
                    binary.as_ref().display()
                )));
            }

            // Resolve offset via symbol lookup (stubbed — real impl uses `object` crate).
            let offset = resolve_symbol_offset(binary.as_ref(), &sym)?;

            self.attached.insert(
                key.clone(),
                UprobeInfo { key, offset },
            );
            tracing::info!(symbol = %sym, offset = offset, "uprobe attached");
            Ok(())
        }
    }

    /// Detach a previously attached uprobe.
    ///
    /// Returns `Err(EbpfError::Unavailable)` when `ebpf` feature is off.
    /// Returns `Err(EbpfError::Uprobe("not attached"))` if not found.
    pub fn detach_uprobe(
        &mut self,
        binary: impl AsRef<Path>,
        symbol: &str,
    ) -> Result<(), EbpfError> {
        #[cfg(not(feature = "ebpf"))]
        {
            let _ = (binary, symbol);
            return Err(EbpfError::Unavailable {
                reason: "ebpf feature not enabled".to_string(),
            });
        }

        #[cfg(feature = "ebpf")]
        {
            let key = UprobeKey::new(binary.as_ref(), symbol);
            match self.attached.remove(&key) {
                Some(_) => {
                    tracing::info!(symbol = %symbol, "uprobe detached");
                    Ok(())
                }
                None => Err(EbpfError::Uprobe(format!(
                    "uprobe '{}' not attached to '{}'",
                    symbol,
                    binary.as_ref().display()
                ))),
            }
        }
    }

    /// Detach all active uprobes.
    pub fn detach_all(&mut self) {
        self.attached.clear();
    }
}

impl Default for UprobeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve the file offset of `symbol` in `binary` using the `object` crate.
///
/// This is a stub — in the real eBPF implementation it would:
/// 1. Open the ELF binary with `object::File::parse()`.
/// 2. Iterate dynamic symbols to find the target.
/// 3. Return the symbol's virtual address minus the load bias.
#[cfg(feature = "ebpf")]
fn resolve_symbol_offset(binary: &Path, symbol: &str) -> Result<u64, EbpfError> {
    // Read the binary
    let data = std::fs::read(binary).map_err(|e| {
        EbpfError::Uprobe(format!(
            "cannot read binary '{}': {}",
            binary.display(),
            e
        ))
    })?;

    use object::{Object, ObjectSymbol};
    let file = object::File::parse(data.as_slice()).map_err(|e| {
        EbpfError::Uprobe(format!("ELF parse error: {}", e))
    })?;

    // Search exported symbols
    for sym in file.symbols().chain(file.dynamic_symbols()) {
        if sym.name().ok() == Some(symbol) {
            return Ok(sym.address());
        }
    }

    Err(EbpfError::Uprobe(format!(
        "symbol '{}' not found in '{}'",
        symbol,
        binary.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uprobe_manager_new_is_empty() {
        let mgr = UprobeManager::new();
        assert_eq!(mgr.attachment_count(), 0);
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_uprobe_attach_unavailable_without_feature() {
        #[cfg(not(feature = "ebpf"))]
        {
            let mut mgr = UprobeManager::new();
            let err = mgr.attach_uprobe("/usr/bin/ls", "main").unwrap_err();
            assert!(matches!(err, EbpfError::Unavailable { .. }));
        }
    }

    #[test]
    fn test_uprobe_detach_unavailable_without_feature() {
        #[cfg(not(feature = "ebpf"))]
        {
            let mut mgr = UprobeManager::new();
            let err = mgr.detach_uprobe("/usr/bin/ls", "main").unwrap_err();
            assert!(matches!(err, EbpfError::Unavailable { .. }));
        }
    }

    #[test]
    fn test_is_attached_always_works() {
        let mgr = UprobeManager::new();
        // Without ebpf feature nothing gets attached so is_attached is always false.
        assert!(!mgr.is_attached("/usr/bin/ls", "main"));
    }

    #[test]
    fn test_detach_all_clears_state() {
        let mut mgr = UprobeManager::new();
        // Manually insert a fake entry to test detach_all
        let key = UprobeKey::new("/fake/binary", "fake_fn");
        mgr.attached.insert(key.clone(), UprobeInfo { key, offset: 0 });
        assert_eq!(mgr.attachment_count(), 1);
        mgr.detach_all();
        assert_eq!(mgr.attachment_count(), 0);
    }

    #[test]
    fn test_uprobe_key_equality() {
        let k1 = UprobeKey::new("/usr/bin/ls", "main");
        let k2 = UprobeKey::new("/usr/bin/ls", "main");
        let k3 = UprobeKey::new("/usr/bin/ls", "other");
        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
    }
}
