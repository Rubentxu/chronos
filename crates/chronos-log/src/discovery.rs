//! REC-C1.5.4 — deterministic discovery of durable execution logs.
//!
//! ## The manifest IS the identity
//!
//! A directory name is a location, not an identity, and the `SessionStore` is a
//! second, unrelated source. The durable `execution-log.manifest.json` already
//! carries the canonical `session_id`, so discovery reads exactly that.
//!
//! ## Two phases, like replay
//!
//! ```text
//! discover -> validate/reopen ALL candidates -> BootstrapPlan -> publish
//! ```
//!
//! Registering as we discover would leave a partially rebuilt registry whose
//! content depends on filesystem order.
//!
//! ## Duplicate identity is refused before anything is published
//!
//! ```text
//! /root/a/manifest -> session_id "foo"
//! /root/b/manifest -> session_id "foo"
//! ```
//!
//! "First one wins" would make the evidence a session reads depend on
//! `read_dir()` order, so neither is published for that identity.

use std::path::{Path, PathBuf};

use crate::record::SessionId;
use crate::segmented::{manifest_path, read_manifest, seg_config_for};

/// A candidate found on disk, with the identity its manifest declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredLog {
    pub dir: PathBuf,
    pub session_id: SessionId,
}

/// A directory that holds segments but no usable manifest.
///
/// Its identity is NOT inferred: the directory name and the first segment's name
/// are both guesses, and C1.5.1's conservative migration only applies when a
/// log is opened explicitly, not when we are trying to reconstruct what exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmanagedLegacyLog {
    pub dir: PathBuf,
    pub reason: String,
}

/// What discovery found, before anything is opened.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveryReport {
    pub logs: Vec<DiscoveredLog>,
    pub unmanaged: Vec<UnmanagedLegacyLog>,
    /// Directories whose manifest could not even be read.
    pub unreadable: Vec<(PathBuf, String)>,
    /// Identities claimed by more than one directory.
    pub duplicates: Vec<(String, Vec<PathBuf>)>,
}

impl DiscoveryReport {
    pub fn has_duplicates(&self) -> bool {
        !self.duplicates.is_empty()
    }
}

/// Scan `root` for execution-log directories, deterministically.
///
/// Ordering is by `session_id`, so the result never depends on `read_dir()`
/// order.
pub fn discover_execution_logs(root: &Path) -> std::io::Result<DiscoveryReport> {
    let mut report = DiscoveryReport::default();
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(report),
        Err(e) => return Err(e),
    };

    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    for dir in dirs {
        let manifest = manifest_path(&dir);
        if !manifest.is_file() {
            // Segments without a manifest: report it, never guess an identity.
            let has_segments = std::fs::read_dir(&dir)
                .map(|rd| {
                    rd.flatten().any(|e| {
                        let n = e.file_name().to_string_lossy().to_string();
                        n.ends_with(".seg")
                    })
                })
                .unwrap_or(false);
            if has_segments {
                report.unmanaged.push(UnmanagedLegacyLog {
                    dir,
                    reason:
                        "segments present without an execution-log manifest; identity not inferred"
                            .to_string(),
                });
            }
            continue;
        }
        match read_manifest(&dir) {
            Ok(Some(m)) if !m.session_id.is_empty() => report.logs.push(DiscoveredLog {
                dir,
                session_id: SessionId::new(m.session_id),
            }),
            Ok(_) => report.unreadable.push((
                dir,
                "manifest present but carries no session identity".to_string(),
            )),
            Err(e) => report.unreadable.push((dir, e.to_string())),
        }
    }

    // Deterministic order: by session id, then by path.
    report.logs.sort_by(|a, b| {
        a.session_id
            .as_str()
            .cmp(b.session_id.as_str())
            .then_with(|| a.dir.cmp(&b.dir))
    });

    // Detect duplicate identities BEFORE any of them is opened.
    let mut i = 0;
    while i < report.logs.len() {
        let id = report.logs[i].session_id.as_str().to_string();
        let start = i;
        while i < report.logs.len() && report.logs[i].session_id.as_str() == id {
            i += 1;
        }
        if i - start > 1 {
            report.duplicates.push((
                id,
                report.logs[start..i]
                    .iter()
                    .map(|l| l.dir.clone())
                    .collect(),
            ));
        }
    }
    // Duplicated identities are not published at all.
    report.logs.retain(|l| {
        !report
            .duplicates
            .iter()
            .any(|(id, _)| id == l.session_id.as_str())
    });

    Ok(report)
}

/// Config for opening a discovered log.
pub fn config_for(dir: &Path) -> crate::segmented::SegmentedConfig {
    seg_config_for(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rec-c1-5-4-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_manifest(dir: &Path, session_id: &str) {
        std::fs::create_dir_all(dir).unwrap();
        let m = crate::segmented::ExecutionLogManifest::new(
            &SessionId::new(session_id),
            crate::seq::EventSeq::ZERO,
        );
        crate::segmented::write_manifest_atomic(dir, &m).unwrap();
    }

    #[test]
    fn boot_3_identity_comes_from_the_manifest_not_the_directory_name() {
        let root = tmpdir("boot3");
        // Directory name is completely unrelated to the identity.
        write_manifest(&root.join("dir-abc"), "session-from-manifest");
        let report = discover_execution_logs(&root).unwrap();
        assert_eq!(report.logs.len(), 1);
        assert_eq!(report.logs[0].session_id.as_str(), "session-from-manifest");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn boot_4_duplicate_identity_publishes_neither() {
        let root = tmpdir("boot4");
        write_manifest(&root.join("a"), "foo");
        write_manifest(&root.join("b"), "foo");
        write_manifest(&root.join("c"), "bar");
        let report = discover_execution_logs(&root).unwrap();
        assert!(report.has_duplicates());
        assert_eq!(report.duplicates[0].0, "foo");
        assert_eq!(report.duplicates[0].1.len(), 2);
        assert_eq!(
            report.logs.len(),
            1,
            "only the unambiguous identity remains"
        );
        assert_eq!(report.logs[0].session_id.as_str(), "bar");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn discovery_order_is_independent_of_filesystem_order() {
        let root = tmpdir("order");
        for (dir, id) in [("z", "s-c"), ("a", "s-a"), ("m", "s-b")] {
            write_manifest(&root.join(dir), id);
        }
        let report = discover_execution_logs(&root).unwrap();
        let ids: Vec<&str> = report.logs.iter().map(|l| l.session_id.as_str()).collect();
        assert_eq!(ids, vec!["s-a", "s-b", "s-c"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn boot_10_segments_without_a_manifest_are_unmanaged_not_inferred() {
        let root = tmpdir("boot10");
        let dir = root.join("legacy");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("legacy-0.seg"), b"not-a-segment").unwrap();
        let report = discover_execution_logs(&root).unwrap();
        assert!(report.logs.is_empty(), "no identity may be inferred");
        assert_eq!(report.unmanaged.len(), 1);
        assert!(report.unmanaged[0].reason.contains("identity not inferred"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_root_is_an_empty_discovery_not_an_error() {
        let root = tmpdir("missing-root");
        let report = discover_execution_logs(&root).unwrap();
        assert!(report.logs.is_empty());
        assert!(report.unmanaged.is_empty());
    }
}
