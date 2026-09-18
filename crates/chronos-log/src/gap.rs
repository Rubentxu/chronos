//! Compatibility re-exports (REC-C3.3.1).
//!
//! `Gap` / `GapReason` semantics are owned by `chronos_domain::evidence`
//! because the storage port (`chronos_domain::ports::execution_log`)
//! needs them in its signature. The inherent impls (`Gap::new`,
//! `Gap::covers`, `Display for GapReason`) live on the canonical
//! domain types — they cannot remain here without breaking the
//! orphan/inherent-impl rule. `chronos_log::gap::Gap` and
//! `chronos_log::gap::GapReason` therefore resolve to the **same**
//! types as `chronos_domain::{Gap, GapReason}`.

pub use chronos_domain::{Gap, GapReason};
