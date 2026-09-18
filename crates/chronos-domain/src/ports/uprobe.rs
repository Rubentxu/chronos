//! `UprobeInjector` / `UprobeHandle` — domain-side port for dynamic
//! uprobe attachment on an existing live probe session.
//!
//! REC-C3.3.2.3 — the capability was previously exposed through
//! `chronos_ebpf::EbpfAdapter::new()` directly inside `ProbeService::inject`,
//! coupling services to the infrastructure crate. This port inverts the
//! dependency: services consume `Arc<dyn UprobeInjector>` from the
//! composition root, and the eBPF backend lives behind a `UprobeHandle`
//! that the session can call without naming the concrete adapter.
//!
//! ## Why split `acquire` from `attach`
//!
//! Three terminal outcomes must be distinguishable in `ProbeInjectResult`:
//!
//! 1. **EbpfUnavailable** — the injector could not even construct a
//!    handle (kernel lacks eBPF, missing CAP_BPF, feature flag off).
//!    In this case there is NO handle to keep.
//! 2. **AttachFailed** — the handle was constructed (the capability
//!    exists) but attaching to the requested symbol failed. The handle
//!    IS retained on the session so its lifecycle is observable and
//!    `probe_status.adapter_owned` continues to reflect the truth.
//! 3. **Attached** — the handle was constructed and the attach succeeded.
//!    Handle is retained.
//!
//! Folding `acquire` and `attach` into a single `injector.attach(...)`
//! call would lose the distinction between (1) and (2): in the (1)
//! failure mode there is no handle at all, so a method returning
//! `Result<Arc<dyn UprobeHandle>, _>` from `attach` would have to
//! fabricate a struct or drop the handle, both of which are wrong.
//! Splitting the two operations makes the three states obvious in
//! the type and impossible to confuse at the call site.
//!
//! ## Why `&self` on `attach`
//!
//! The handle behind `Arc<dyn UprobeHandle>` is meant to be shared, and
//! the concrete eBPF adapter uses interior mutability (`Mutex<Inner>`)
//! for its internal state. Pinning `attach` to `&self` lets multiple
//! call sites hold clones of the `Arc`, while the underlying state
//! updates serially through its own mutex.
//!
//! ## Why no eBPF-specific methods
//!
//! The port represents the **capability services need** — attach a
//! uprobe to a process — not an abstract version of `EbpfAdapter`. Things
//! like `is_ebpf_available`, `adapter_name`, or `raw_link` belong on a
//! concrete adapter behind the port, never on the port itself. Future
//! backends (patches, Frida, etc.) fit the same shape without forcing
//! services to grow a fattening compatibility surface.
//!
//! Tests for this module live under
//! `crates/chronos-domain/tests/ports/uprobe.rs`.

use std::sync::Arc;

use crate::capability::CapabilityUnavailable;

/// Acquire (or refuse to acquire) a handle to the uprobe capability.
///
/// `acquire` is the **detection point** for whether the host satisfies
/// the uprobe capability (kernel >= 5.8, CAP_BPF, feature flag on, …).
/// When it returns `Err`, the caller knows there is no handle to keep
/// and surfaces the failure as `ProbeInjectResult::EbpfUnavailable`.
///
/// When it returns `Ok`, the handle is constructed **but the kernel
/// uprobe has NOT been attached yet**. The caller drives the next step
/// with [`UprobeHandle::attach`]; only successful `attach` produces a
/// probe that emits events.
pub trait UprobeInjector: Send + Sync {
    /// Build a fresh handle for the uprobe capability.
    fn acquire(&self) -> Result<Arc<dyn UprobeHandle>, CapabilityUnavailable>;
}

/// One live uprobe handle, produced by [`UprobeInjector::acquire`].
///
/// `attach` materializes the kernel uprobe on `(pid, binary_path, symbol_name)`.
/// `detach` is its inverse, invoked from `probe_stop` so the kernel
/// uprobe is removed explicitly (not waiting on `Drop`).
///
/// ## Why retain the handle on attach failure
///
/// In the legacy `EbpfAdapter`-directly-in-services design, the adapter
/// was stored on the session even when `attach_uprobe` returned an
/// error, so `probe_status` could still distinguish "kernel has the
/// feature" (handle exists) from "the requested symbol could not be
/// resolved" (no kernel uprobe). This port preserves that semantics:
/// services retain the handle on both `Ok(())` and `Err(_)` from
/// `attach`, and the `ProbeInjectResult` is the only thing that
/// diverges between them.
pub trait UprobeHandle: Send + Sync {
    /// Attach the uprobe to `pid` / `binary_path` / `symbol_name`.
    ///
    /// On `Ok(())` the kernel uprobe is live. On `Err(UprobeAttachError)`
    /// the handle remains valid (other methods on the same handle may
    /// still be callable) but no kernel uprobe was created for this
    /// particular attach call.
    fn attach(
        &self,
        pid: u32,
        binary_path: &str,
        symbol_name: &str,
    ) -> Result<(), UprobeAttachError>;

    /// Detach any kernel uprobes this handle owns.
    ///
    /// Idempotent: calling `detach` twice on the same handle is a no-op
    /// (the second call may be observed as `Ok(())` or with a benign
    /// "already detached" error — see the concrete adapter contract).
    fn detach(&self) -> Result<(), UprobeAttachError>;
}

/// Concrete error from `UprobeHandle::attach` / `UprobeHandle::detach`.
///
/// Distinct from [`CapabilityUnavailable`]: the capability exists
/// (otherwise [`UprobeInjector::acquire`] would have returned `Err`).
/// Failures here mean the kernel refused the specific operation:
/// unresolved symbol, target pid is gone, BPF map full, etc. The
/// human-readable `detail` carries the underlying adapter message.
///
/// Deliberately not a `thiserror` enum: the underlying adapter already
/// enumerates its own rich error type, and the port's contract is "one
/// detail string per failure". If a port-side call site ever needs to
/// distinguish subclasses, the discrimination moves into the adapter
/// without changing the port surface.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("uprobe attach failed: {detail}")]
pub struct UprobeAttachError {
    pub detail: String,
}

impl UprobeAttachError {
    /// Build from any stringy error type.
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}
