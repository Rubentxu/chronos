//! Capability contracts for time-travel debugging primitives.
//!
//! Some Chronos operations require elevated OS capabilities (kernel features,
//! privileges, or external binaries) that may not be available on every host.
//! The `CapabilityUnavailable` enum enumerates the discrete capability slots
//! that the system reasons about, and is the typed contract returned to
//! callers when a slot cannot be activated in the current environment.
//!
//! ## Why this lives in `chronos-domain`
//!
//! The set of capability slots is a product contract — both the agent-facing
//! surface (the MCP tool family) and the implementation crates
//! (`chronos-ebpf`, `chronos-native`) need to agree on what a "missing
//! capability" means. Centralising the enum in the domain crate avoids
//! stringly-typed drift between layers and gives every boundary a
//! pattern-matchable, `Copy`, `Eq` discriminator.

use thiserror::Error;

/// Identifies an OS / runtime capability required by a Chronos operation.
///
/// `CapabilityUnavailable` is the typed reason emitted when a probe or
/// capture path cannot run on the current host. Every variant maps to a
/// specific detection point; downstream code should pattern-match on the
/// discriminator rather than inspecting the human-readable detail string.
///
/// ## Adding a new variant
///
/// 1. Decide whether the new capability slot is orthogonal to the existing
///    ones (it should be — each variant represents one detection point).
/// 2. Update the capability snapshot at `McpServer::capability_snapshot`
///    (chronos-mcp) so users can see which slots are active.
/// 3. Add a `#[cfg(test)]` mapping in `chronos-services/src/capability.rs`
///    from the adapter-level error to this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// eBPF uprobe injection. Requires `CAP_BPF` + `CAP_PERFMON` (or root)
    /// and a kernel >= 5.8 (ring buffer support). Without the `ebpf`
    /// feature in `chronos-ebpf`, the adapter is permanently unavailable.
    EbpfUprobe,

    /// ptrace attach. Requires `CAP_SYS_PTRACE` (or root) and same-uid
    /// access to the target process. Used by `session_start{action=attach}`.
    PtraceAttach,

    /// REC-C3.3.2.4: browser/CDP probe. Requires Chrome (or Chromium) on
    /// the host PATH, or an explicit `chrome_path` override. Used by
    /// `browser_probe_start`.
    BrowserProbe,
}

impl Capability {
    /// Stable kebab-case identifier (used in MCP error responses and JSON).
    pub fn as_str(&self) -> &'static str {
        match self {
            Capability::EbpfUprobe => "ebpf-uprobe",
            Capability::PtraceAttach => "ptrace-attach",
            Capability::BrowserProbe => "browser-probe",
        }
    }

    /// Human-readable explanation of the capability, used in tool error text.
    pub fn description(&self) -> &'static str {
        match self {
            Capability::EbpfUprobe => {
                "eBPF uprobe injection (requires root or CAP_BPF/CAP_PERFMON, kernel >= 5.8)"
            }
            Capability::PtraceAttach => {
                "ptrace attach (requires root or CAP_SYS_PTRACE, same-uid target)"
            }
            Capability::BrowserProbe => {
                "browser/CDP probe (requires Chrome or Chromium on host PATH)"
            }
        }
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Typed "capability unavailable" error.
///
/// Returned when a Chronos operation requires a capability that the
/// current host does not satisfy. The `capability` field is the
/// machine-readable discriminator; `reason` carries the underlying
/// adapter-level message (kernel error, feature flag, …) and is intended
/// for humans.
///
/// Use `CapabilityUnavailable::capability()` to pattern-match the slot
/// without inspecting `reason`.
#[derive(Debug, Clone, Error)]
#[error("capability unavailable: {capability} ({reason})")]
pub struct CapabilityUnavailable {
    /// Which capability slot is missing.
    pub capability: Capability,
    /// Human-readable detail (kernel error string, feature flag note, …).
    pub reason: String,
}

impl CapabilityUnavailable {
    /// Build a `CapabilityUnavailable` for the eBPF uprobe slot.
    pub fn ebpf_uprobe(reason: impl Into<String>) -> Self {
        Self {
            capability: Capability::EbpfUprobe,
            reason: reason.into(),
        }
    }

    /// Build a `CapabilityUnavailable` for the ptrace attach slot.
    pub fn ptrace_attach(reason: impl Into<String>) -> Self {
        Self {
            capability: Capability::PtraceAttach,
            reason: reason.into(),
        }
    }

    /// REC-C3.3.2.4 — build a `CapabilityUnavailable` for the
    /// browser/CDP probe slot (Chrome on host PATH).
    pub fn browser_probe(reason: impl Into<String>) -> Self {
        Self {
            capability: Capability::BrowserProbe,
            reason: reason.into(),
        }
    }

    /// Which capability slot is missing.
    pub fn capability(&self) -> Capability {
        self.capability
    }
}

impl PartialEq for CapabilityUnavailable {
    fn eq(&self, other: &Self) -> bool {
        self.capability == other.capability
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_kebab_strings_are_stable() {
        assert_eq!(Capability::EbpfUprobe.as_str(), "ebpf-uprobe");
        assert_eq!(Capability::PtraceAttach.as_str(), "ptrace-attach");
    }

    #[test]
    fn capability_unavailable_carries_typed_slot() {
        let err = CapabilityUnavailable::ebpf_uprobe("feature flag off");
        assert_eq!(err.capability(), Capability::EbpfUprobe);
        assert_eq!(err.reason, "feature flag off");
        assert_eq!(
            err.to_string(),
            "capability unavailable: ebpf-uprobe (feature flag off)"
        );
    }

    #[test]
    fn capability_unavailable_equality_is_by_slot_only() {
        // Two errors on the same slot are equal regardless of the human
        // reason — important so callers can compare typed errors without
        // caring about exact wording.
        let a = CapabilityUnavailable::ebpf_uprobe("kernel too old");
        let b = CapabilityUnavailable::ebpf_uprobe("feature flag off");
        assert_eq!(a, b);
    }
}
