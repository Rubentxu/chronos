//! REC-C1.5.4 — registry bootstrap from validated durable state.
//!
//! ## Two phases
//!
//! ```text
//! discover -> reopen_existing ALL candidates -> BootstrapPlan -> publish
//! ```
//!
//! Registering as we discover would leave a partially rebuilt registry whose
//! content depended on filesystem order.
//!
//! ## A corrupt session does not stop the server
//!
//! One unreadable log must not prevent Chronos from starting, and it must not be
//! hidden either:
//!
//! ```text
//! A valid     -> Available
//! B corrupt   -> Unavailable(reason)
//! C valid     -> Available
//! ```
//!
//! `events_read(B)` then reports `ExecutionLogUnavailable(reason)`, never
//! `SessionNotFound` and never a fallback.
//!
//! ## No read-time reopen
//!
//! Bootstrap happens once, before serving reads. There is no `get_or_reopen` on
//! the read path: that would let `drop_session` be undone by the next read and
//! would make a durable session flicker between unavailable and available
//! without anything external changing.

use std::path::{Path, PathBuf};

use chronos_log::SessionId;
use chronos_log::{discover_execution_logs, UnmanagedLegacyLog};

use crate::error::ServiceError;
use crate::session_log::{SessionExecutionLog, SessionExecutionLogRegistry};

/// One bootstrap outcome for an identity we could name.
#[derive(Debug, Clone)]
pub enum BootstrapEntry {
    /// Carries the ALREADY-VALIDATED handle.
    ///
    /// Reopening during `apply` would reintroduce a TOCTOU window: the plan could
    /// say `Available` while the registry ends up `Unavailable` because the
    /// filesystem changed in between. The plan must be exactly what is published.
    Available {
        session_id: String,
        dir: PathBuf,
        log: SessionExecutionLog,
    },
    Unavailable {
        session_id: String,
        dir: PathBuf,
        reason: String,
    },
}

/// Everything bootstrap concluded, before anything is published.
#[derive(Debug, Clone, Default)]
pub struct BootstrapPlan {
    pub entries: Vec<BootstrapEntry>,
    /// Directories with segments but no usable manifest: reported by path,
    /// because there is no trustworthy identity to register them under.
    pub unmanaged: Vec<UnmanagedLegacyLog>,
    /// Directories whose manifest could not be read at all.
    pub unreadable: Vec<(PathBuf, String)>,
    /// Identities claimed by more than one directory: none of them is published.
    pub duplicates: Vec<(String, Vec<PathBuf>)>,
}

impl BootstrapPlan {
    pub fn available(&self) -> impl Iterator<Item = &BootstrapEntry> {
        self.entries
            .iter()
            .filter(|e| matches!(e, BootstrapEntry::Available { .. }))
    }

    pub fn unavailable(&self) -> impl Iterator<Item = &BootstrapEntry> {
        self.entries
            .iter()
            .filter(|e| matches!(e, BootstrapEntry::Unavailable { .. }))
    }
}

/// Phase 1: discover and validate every candidate. Nothing is published.
pub fn build_bootstrap_plan(root: &Path) -> Result<BootstrapPlan, ServiceError> {
    let discovery = discover_execution_logs(root)
        .map_err(|e| ServiceError::DrainFailed(format!("discovery of {root:?}: {e}")))?;

    let mut plan = BootstrapPlan {
        unmanaged: discovery.unmanaged,
        unreadable: discovery.unreadable,
        duplicates: discovery.duplicates,
        entries: Vec::new(),
    };

    for found in discovery.logs {
        let session_id = found.session_id.clone();
        match SessionExecutionLog::reopen_existing(&found.dir, SessionId::new(session_id.as_str()))
        {
            Ok(log) => plan.entries.push(BootstrapEntry::Available {
                session_id: session_id.as_str().to_string(),
                dir: found.dir,
                log,
            }),
            Err(e) => plan.entries.push(BootstrapEntry::Unavailable {
                session_id: session_id.as_str().to_string(),
                dir: found.dir,
                reason: e.to_string(),
            }),
        }
    }
    Ok(plan)
}

/// Phase 2: publish a validated plan into the registry.
///
/// No IO and no reopen: the plan already holds the validated handles, so what is
/// published is exactly what was validated. Reopening here would reopen the
/// TOCTOU window where the plan says `Available` and the registry ends up
/// `Unavailable`.
pub fn apply_bootstrap_plan(
    registry: &SessionExecutionLogRegistry,
    plan: &BootstrapPlan,
) -> Result<(), ServiceError> {
    for entry in &plan.entries {
        match entry {
            BootstrapEntry::Available { log, .. } => registry.register(log.clone())?,
            BootstrapEntry::Unavailable {
                session_id, reason, ..
            } => registry.register_unavailable(session_id, reason.clone())?,
        }
    }
    Ok(())
}

/// Convenience: build and apply in one call (still two phases internally).
pub fn bootstrap_execution_logs(
    root: &Path,
    registry: &SessionExecutionLogRegistry,
) -> Result<BootstrapPlan, ServiceError> {
    let plan = build_bootstrap_plan(root)?;
    apply_bootstrap_plan(registry, &plan)?;
    Ok(plan)
}

/// Durably remove a session's execution log (REC-C1.5.4 `delete_session`).
///
/// `drop_session` only forgets the in-memory handle: a restart legitimately
/// rediscovers it. `delete_session` must make the session stop being
/// discoverable, otherwise the next bootstrap resurrects it.
///
/// This uses PURE DISCOVERY, never `build_bootstrap_plan`: reopening a session
/// is not an observational operation (it can recover a persisted `Open` into
/// `Unclean` and persist that), and deleting A must not mutate B.
///
/// Duplicate identities are covered too: discovery removes them from
/// `report.logs` and keeps them in `report.duplicates`, so a delete of an
/// ambiguous identity would otherwise be impossible.
///
/// Partial failures are reported, never hidden: the handle is already out of the
/// registry and anything left on disk would be rediscovered.
pub fn delete_durable_execution_log(
    registry: &SessionExecutionLogRegistry,
    root: &Path,
    session_id: &str,
) -> Result<Vec<PathBuf>, ServiceError> {
    registry.remove(session_id);

    let report = discover_execution_logs(root)
        .map_err(|e| ServiceError::DrainFailed(format!("discovery of {root:?}: {e}")))?;

    let mut targets: Vec<PathBuf> = report
        .logs
        .iter()
        .filter(|l| l.session_id.as_str() == session_id)
        .map(|l| l.dir.clone())
        .collect();
    for (id, paths) in &report.duplicates {
        if id == session_id {
            targets.extend(paths.iter().cloned());
        }
    }
    targets.sort();
    targets.dedup();

    let mut removed = Vec::new();
    let mut failures = Vec::new();
    for dir in targets {
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => removed.push(dir),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => failures.push(format!("{}: {e}", dir.display())),
        }
    }
    if !failures.is_empty() {
        return Err(ServiceError::DrainFailed(format!(
            "durable execution log removal incomplete: {}",
            failures.join("; ")
        )));
    }
    Ok(removed)
}

#[cfg(test)]
mod boot_tests {
    use super::*;
    use chronos_log::{
        NewExecutionRecord, SegmentedConfig, SegmentedExecutionLog, SessionId as LogSessionId,
    };

    fn tmpdir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rec-c1-5-4-boot-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    /// Create a durable log with `n` records under `root/<dir_name>`.
    fn make_log(root: &Path, dir_name: &str, session: &str, n: u64) -> PathBuf {
        let dir = root.join(dir_name);
        let cfg = SegmentedConfig::with_dir(dir.clone());
        let log = SegmentedExecutionLog::open(LogSessionId::new(session), cfg).expect("open");
        for i in 0..n {
            log.append(NewExecutionRecord {
                session_id: LogSessionId::new(session),
                monotonic_ns: i,
                payload: chronos_log::ExecutionPayload::new(format!("r{i}").into_bytes(), "t"),
                invocation_id: None,
                parent_invocation_id: None,
                symbol_id: None,
                captured_at_unix_ns: None,
            })
            .expect("append");
        }
        log.flush().expect("flush");
        dir
    }

    #[test]
    fn boot_1_two_valid_logs_bootstrap_exactly_two() {
        let root = tmpdir("b1");
        make_log(&root, "a", "s-a", 3);
        make_log(&root, "b", "s-b", 5);
        let registry = SessionExecutionLogRegistry::new();
        let plan = bootstrap_execution_logs(&root, &registry).expect("bootstrap");
        assert_eq!(plan.available().count(), 2);
        assert_eq!(registry.len(), 2);
        assert!(registry.get("s-a").is_ok());
        assert!(registry.get("s-b").is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn boot_2_restart_preserves_identities_and_evidence() {
        let root = tmpdir("b2");
        make_log(&root, "a", "s-a", 4);

        let r1 = SessionExecutionLogRegistry::new();
        bootstrap_execution_logs(&root, &r1).expect("first bootstrap");
        let before = r1.get("s-a").expect("available");

        // Simulate a restart: a brand-new registry bootstrapped from disk.
        let r2 = SessionExecutionLogRegistry::new();
        bootstrap_execution_logs(&root, &r2).expect("second bootstrap");
        let after = r2.get("s-a").expect("available after restart");

        assert_eq!(after.session_id(), before.session_id());
        assert_eq!(after.handle().tail_seq(), before.handle().tail_seq());
        let page = after
            .handle()
            .read_from_seq(chronos_log::EventSeq::ZERO, 10)
            .expect("read");
        assert_eq!(page.records.len(), 4, "the same evidence is remembered");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn boot_5_corrupt_log_is_unavailable_with_its_reason() {
        let root = tmpdir("b5");
        let dir = make_log(&root, "bad", "s-bad", 3);
        // Corrupt the only segment's body.
        let seg = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().map(|e| e == "seg").unwrap_or(false))
            .expect("a segment");
        let mut bytes = std::fs::read(&seg).unwrap();
        let n = bytes.len() * 3 / 4;
        bytes[n] ^= 0xFF;
        std::fs::write(&seg, &bytes).unwrap();

        let registry = SessionExecutionLogRegistry::new();
        let plan = bootstrap_execution_logs(&root, &registry).expect("bootstrap");
        assert_eq!(plan.unavailable().count(), 1);
        let err = registry.get("s-bad").expect_err("unavailable");
        match err {
            ServiceError::ExecutionLogUnavailable { reason, .. } => {
                assert!(
                    reason.contains("reopen") || reason.contains("Integrity"),
                    "{reason}"
                );
            }
            other => panic!("expected ExecutionLogUnavailable, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn boot_13_one_corrupt_session_does_not_block_the_others() {
        let root = tmpdir("b13");
        make_log(&root, "a", "s-a", 2);
        let bad = make_log(&root, "b", "s-b", 2);
        make_log(&root, "c", "s-c", 2);
        let seg = std::fs::read_dir(&bad)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().map(|e| e == "seg").unwrap_or(false))
            .unwrap();
        let mut bytes = std::fs::read(&seg).unwrap();
        let n = bytes.len() * 3 / 4;
        bytes[n] ^= 0xFF;
        std::fs::write(&seg, &bytes).unwrap();

        let registry = SessionExecutionLogRegistry::new();
        let plan = bootstrap_execution_logs(&root, &registry).expect("bootstrap");
        // Deterministic: A and C available, B unavailable, regardless of order.
        assert_eq!(plan.available().count(), 2);
        assert_eq!(plan.unavailable().count(), 1);
        assert!(registry.get("s-a").is_ok());
        assert!(registry.get("s-c").is_ok());
        assert!(registry.get("s-b").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn boot_9_disappearing_directory_is_not_recreated_as_an_empty_log() {
        let root = tmpdir("b9");
        make_log(&root, "a", "s-a", 3);
        let plan = build_bootstrap_plan(&root).expect("plan");
        assert_eq!(plan.available().count(), 1);

        // The directory vanishes between discovery and publish.
        let dir = plan
            .entries
            .iter()
            .find_map(|e| match e {
                BootstrapEntry::Available { dir, .. } => Some(dir.clone()),
                _ => None,
            })
            .expect("a dir");
        let handle_tail = plan.entries.iter().find_map(|e| match e {
            BootstrapEntry::Available { log, .. } => log.handle().tail_seq(),
            _ => None,
        });
        std::fs::remove_dir_all(&dir).expect("remove");

        let registry = SessionExecutionLogRegistry::new();
        apply_bootstrap_plan(&registry, &plan).expect("apply");

        // The anti-lie property: no empty log is CREATED on disk, and the tail
        // the plan validated is preserved. The registry publishes exactly the
        // validated plan (that is the point of carrying handles).
        assert!(!dir.exists(), "an empty log must never be created");
        let published = registry.get("s-a").expect("plan is what gets published");
        assert_eq!(published.handle().tail_seq(), handle_tail);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn boot_11_drop_forgets_but_restart_rediscovers() {
        let root = tmpdir("b11");
        make_log(&root, "a", "s-a", 2);
        let registry = SessionExecutionLogRegistry::new();
        bootstrap_execution_logs(&root, &registry).expect("bootstrap");

        // drop_session: in-memory only.
        registry.remove("s-a");
        assert!(registry.get("s-a").is_err());

        // A restart rediscovers it, because the durable log was never deleted.
        let after_restart = SessionExecutionLogRegistry::new();
        bootstrap_execution_logs(&root, &after_restart).expect("rebootstrap");
        assert!(after_restart.get("s-a").is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_publishes_exactly_the_validated_plan_and_touches_no_filesystem() {
        let root = tmpdir("toctou");
        make_log(&root, "a", "s-a", 2);
        make_log(&root, "b", "s-b", 2);
        let plan = build_bootstrap_plan(&root).expect("plan");
        assert_eq!(plan.available().count(), 2);

        // Mutate the filesystem between plan and apply: the plan must still win,
        // because it is the conclusion we validated.
        for entry in &plan.entries {
            if let BootstrapEntry::Available { dir, .. } = entry {
                let _ = std::fs::remove_dir_all(dir);
            }
        }

        let registry = SessionExecutionLogRegistry::new();
        apply_bootstrap_plan(&registry, &plan).expect("apply");
        assert!(
            registry.get("s-a").is_ok() && registry.get("s-b").is_ok(),
            "the published registry must equal the validated plan: {:?} / {:?}",
            registry.get("s-a").err(),
            registry.get("s-b").err()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn delete_does_not_mutate_other_sessions() {
        let root = tmpdir("delete-isolated");
        make_log(&root, "a", "s-a", 2);
        make_log(&root, "b", "s-b", 2);

        // Bootstrap itself legitimately recovers B (a persisted Open from a run
        // that never sealed). The property under test is that the DELETE does not
        // touch B, so the baseline is taken after bootstrap.
        let registry = SessionExecutionLogRegistry::new();
        bootstrap_execution_logs(&root, &registry).expect("bootstrap");
        let before = chronos_log::segmented::read_manifest(&root.join("b"))
            .unwrap()
            .expect("manifest");
        delete_durable_execution_log(&registry, &root, "s-a").expect("delete A");
        let after = chronos_log::segmented::read_manifest(&root.join("b"))
            .unwrap()
            .expect("manifest");
        assert_eq!(
            after.tail_state, before.tail_state,
            "deleting A must not change B's tail state"
        );
        assert_eq!(after.retained_from, before.retained_from);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn delete_covers_a_duplicated_identity() {
        let root = tmpdir("delete-dup");
        for dir in ["a", "b"] {
            let d = root.join(dir);
            std::fs::create_dir_all(&d).unwrap();
            let m = chronos_log::segmented::ExecutionLogManifest::new(
                &LogSessionId::new("dup"),
                chronos_log::EventSeq::ZERO,
            );
            chronos_log::segmented::write_manifest_atomic(&d, &m).unwrap();
        }
        let registry = SessionExecutionLogRegistry::new();
        let removed =
            delete_durable_execution_log(&registry, &root, "dup").expect("delete ambiguous");
        assert_eq!(removed.len(), 2, "both locations are removed");
        assert!(discover_execution_logs(&root)
            .unwrap()
            .duplicates
            .is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn boot_12_delete_makes_the_session_stop_being_discoverable() {
        let root = tmpdir("b12");
        make_log(&root, "a", "s-a", 2);
        make_log(&root, "b", "s-b", 2);
        let registry = SessionExecutionLogRegistry::new();
        bootstrap_execution_logs(&root, &registry).expect("bootstrap");
        assert!(registry.get("s-a").is_ok());

        let removed = delete_durable_execution_log(&registry, &root, "s-a").expect("delete");
        assert!(!removed.is_empty(), "the durable directory is gone");
        assert!(registry.get("s-a").is_err());

        // A restart must NOT resurrect it.
        let after_restart = SessionExecutionLogRegistry::new();
        bootstrap_execution_logs(&root, &after_restart).expect("rebootstrap");
        assert!(
            after_restart.get("s-a").is_err(),
            "delete_session must be durable"
        );
        assert!(after_restart.get("s-b").is_ok(), "other sessions survive");
        let _ = std::fs::remove_dir_all(&root);
    }
}
