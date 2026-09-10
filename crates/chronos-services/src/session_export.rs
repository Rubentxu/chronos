//! M6 — Session Export dispatcher (m6-05).
//!
//! The v2 `session_export` tool is the second of two net-new capabilities
//! identified in `docs/milestones/m5-close-report.md` §4.2 (alongside
//! `hypothesis_test` shipped in m6-04). There is **no v1 tool** to
//! deprecate; this cycle ships a brand-new dispatcher + DTOs + MCP wrapper.
//!
//! The dispatcher serializes a [`ExportBundle`] (metadata + trace events +
//! properties snapshot) to disk in one of two wire formats:
//!
//! - [`ExportFormat::Json`] -- pretty-printed canonical bundle, round-
//!   trippable via `serde_json::from_slice`.
//! - [`ExportFormat::OtlpJson`] -- OpenTelemetry-compatible JSON wire
//!   format (`{"resourceSpans":[{...}]}`). Compatible with Jaeger,
//!   Tempo, and Honeycomb JSON receivers. We do NOT ship full OTLP/gRPC
//!   proto in m6-05 (no protobuf dependencies pulled in).
//! - [`ExportFormat::ZipJson`] -- reserved for m7+; the variant is
//!   declared but the dispatcher rejects it with `InvalidExportParameter`
//!   to keep m6-05 honest about scope.
//!
//! The write is atomic: the bundle is serialized into a `<path>.tmp.<pid>`
//! file, fsync'd, then renamed onto the final path. A failure during
//! serialization or write leaves no partial file behind at the final
//! location.
//!
//! # Known limitation: `properties_snapshot` is always `Vec::new()`
//!
//! The `QueryEngine` API exposes events and indices but does not currently
//! return a property table view (properties are evaluated on-demand by
//! `property.observe` name, see `chronos_query::PropertyProjection::run`).
//! Adding a `QueryEngine::properties()` accessor is a separable concern
//! (probably M7+). For m6-05 the bundle's `properties_snapshot` field is
//! always empty. The DTO + JSON schema are final so consumers don't need
//! to special-case absent-vs-empty payloads once M7+ fills it in.
//!
//! See `docs/milestones/m6-05-session-export.md` for the full spec and
//! test matrix.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chronos_domain::{Language, TraceEvent};
use chronos_query::QueryEngine;
use chronos_store::SessionMetadata;
use serde_json::json;
use tokio::sync::Mutex as TokioMutex;

use crate::error::ServiceError;
use crate::output::{ExportBundle, ExportFormat, ExportResult};

/// Borrowed handle to the live engine map (shared with the MCP server).
///
/// Matches the established pattern in `chronos_services::trace_slice`,
/// `state_query`, `execution_query`, `hypothesis_test`, etc. — the value
/// type is `QueryEngine` (no inner `Arc`); `Arc`-wrapping happens at the
/// call site.
pub struct SessionExportContext<'a> {
    pub engines: &'a TokioMutex<HashMap<String, QueryEngine>>,
}

/// Stateless holder for the v2 `session_export` dispatcher.
#[derive(Debug, Default)]
pub struct ChronosSessionExportService;

impl ChronosSessionExportService {
    /// Export a session bundle to disk in the requested format.
    ///
    /// Flow:
    /// 1. Lock engines, fetch the engine for `session_id`. Error:
    ///    `SessionNotInMemory`.
    /// 2. Pull `engine.get_all_events()`. Error: `EmptySession` if 0.
    /// 3. Build `SessionMetadata` (mirrors `SessionsService::save_session`).
    /// 4. Build `ExportBundle` with `properties_snapshot = Vec::new()`
    ///    (see module docs — known limitation of m6-05).
    /// 5. Serialize the bundle in the requested format (Json / OtlpJson).
    ///    ZipJson is rejected with `InvalidExportParameter`.
    /// 6. Atomic write to `<output_path>.tmp.<pid>` then rename.
    ///    Failure at any io step returns `ExportFailed`.
    ///
    /// The lock is released before step 5+6 (no async dance needed; the
    /// engine is dropped after `get_all_events()`).
    pub async fn export(
        session_id: &str,
        language: Language,
        target: String,
        format: ExportFormat,
        output_path: &Path,
        ctx: &SessionExportContext<'_>,
    ) -> Result<ExportResult, ServiceError> {
        // Reject reserved format variants up-front so callers fail fast.
        if matches!(format, ExportFormat::ZipJson) {
            return Err(ServiceError::InvalidExportParameter(
                "export_format 'zip_json' is reserved for a future cycle; use 'json' or 'otlp_json'"
                    .to_string(),
            ));
        }

        // Step 1+2+3: snapshot the engine state under the lock.
        let (metadata, events): (SessionMetadata, Vec<TraceEvent>) = {
            let guard = ctx.engines.lock().await;
            let engine = guard
                .get(session_id)
                .ok_or_else(|| ServiceError::SessionNotInMemory(session_id.to_string()))?;

            let events = engine.get_all_events();
            let event_count = events.len();
            if event_count == 0 {
                return Err(ServiceError::EmptySession(session_id.to_string()));
            }

            let (duration_ms, created_at) =
                if let (Some(first), Some(last)) = (events.first(), events.last()) {
                    let dur_ns = last.timestamp_ns.saturating_sub(first.timestamp_ns);
                    (dur_ns / 1_000_000, last.timestamp_ns / 1_000_000)
                } else {
                    (0, 0)
                };

            let metadata = SessionMetadata {
                session_id: session_id.to_string(),
                created_at,
                language: language.to_string(),
                target: target.clone(),
                event_count,
                duration_ms,
            };
            (metadata, events)
        };

        // Step 4: assemble the bundle. `properties_snapshot` is empty
        // in m6-05 -- see module docs.
        let bundle = ExportBundle {
            schema_version: "v2-export.1".to_string(),
            metadata,
            events,
            properties_snapshot: Vec::new(),
        };

        // Step 5: serialize in the requested format.
        let bytes = match format {
            ExportFormat::Json => serialize_bundle_json(&bundle)?,
            ExportFormat::OtlpJson => serialize_bundle_otlp_json(&bundle)?,
            ExportFormat::ZipJson => unreachable!("rejected above"),
        };

        // Step 6: atomic write (tmp + rename).
        let final_path = atomic_write(output_path, &bytes)?;

        Ok(ExportResult {
            path: final_path,
            bytes_written: bytes.len() as u64,
            format,
        })
    }
}

/// Serialize the bundle in the canonical Json format (pretty-printed).
fn serialize_bundle_json(bundle: &ExportBundle) -> Result<Vec<u8>, ServiceError> {
    serde_json::to_vec_pretty(bundle).map_err(|e| ServiceError::ExportFailed(e.to_string()))
}

/// Serialize the bundle in OTLP-compatible Json wire format.
///
/// Shape: `{"resourceSpans": [{"resource": {"attributes": [...]}, "scopeSpans": [{"scope": {...}, "spans": [...]}]}]}`.
/// - `resource.attributes` carries session metadata + schema_version.
/// - `scope.attributes` carries properties_snapshot (empty in m6-05).
/// - Each `TraceEvent` becomes one OTLP `Span` with `name` = event
///   type, `start_time_unix_nano` = event.timestamp_ns, and
///   `attributes` carrying the event's `data` as a JSON object.
fn serialize_bundle_otlp_json(bundle: &ExportBundle) -> Result<Vec<u8>, ServiceError> {
    let resource_attrs = json!([
        { "key": "schema.version",       "value": { "stringValue": bundle.schema_version } },
        { "key": "session.id",           "value": { "stringValue": bundle.metadata.session_id } },
        { "key": "session.language",     "value": { "stringValue": bundle.metadata.language } },
        { "key": "session.target",       "value": { "stringValue": bundle.metadata.target } },
        { "key": "session.event_count",  "value": { "intValue": bundle.metadata.event_count as i64 } },
        { "key": "session.duration_ms",  "value": { "intValue": bundle.metadata.duration_ms as i64 } },
    ]);

    let scope_attrs = json!(bundle
        .properties_snapshot
        .iter()
        .map(|p| json!({
            "key": p.name,
            "value": { "stringValue": format!("{:?}", p.value) },
        }))
        .collect::<Vec<_>>());

    let spans: Vec<_> = bundle
        .events
        .iter()
        .map(|ev| {
            json!({
                "name": format!("{:?}", ev.event_type),
                "start_time_unix_nano": ev.timestamp_ns.to_string(),
                "end_time_unix_nano":   ev.timestamp_ns.to_string(),
                "attributes": [{
                    "key": "chronos.event_id",
                    "value": { "intValue": ev.event_id as i64 },
                }, {
                    "key": "chronos.thread_id",
                    "value": { "intValue": ev.thread_id as i64 },
                }, {
                    "key": "chronos.event_data",
                    "value": { "stringValue": format!("{:?}", ev.data) },
                }],
            })
        })
        .collect();

    let otlp = json!({
        "resourceSpans": [{
            "resource": { "attributes": resource_attrs },
            "scopeSpans": [{
                "scope": {
                    "name": "chronos.session_export",
                    "version": "v2-export.1",
                    "attributes": scope_attrs,
                },
                "spans": spans,
            }],
        }],
    });

    serde_json::to_vec_pretty(&otlp).map_err(|e| ServiceError::ExportFailed(e.to_string()))
}

/// Write `bytes` to `<output_path>.tmp.<pid>` then rename onto `output_path`.
///
/// Returns the final path on success. On any io failure returns
/// `ExportFailed` carrying the underlying error string. The tmp file is
/// best-effort cleaned up on failure so we don't leak junk in the parent
/// directory.
fn atomic_write(output_path: &Path, bytes: &[u8]) -> Result<PathBuf, ServiceError> {
    let pid = std::process::id();
    let tmp_path = {
        let mut p = output_path.to_path_buf();
        let file_name = p
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "export".to_string());
        p.set_file_name(format!("{file_name}.tmp.{pid}"));
        p
    };

    // Ensure the parent dir exists; fail early with a clear message
    // rather than relying on the open() error.
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(ServiceError::ExportFailed(format!(
                "output_path parent does not exist: {}",
                parent.display()
            )));
        }
    }

    // Write tmp.
    let write_result = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        // Best-effort cleanup of the tmp file.
        let _ = fs::remove_file(&tmp_path);
        return Err(ServiceError::ExportFailed(format!(
            "tmp write to {} failed: {e}",
            tmp_path.display()
        )));
    }

    // Rename onto final path. If rename fails (cross-device, etc.) copy
    // + remove as a fallback so the export still lands somewhere sane.
    if let Err(e) = fs::rename(&tmp_path, output_path) {
        match fs::copy(&tmp_path, output_path).and_then(|_| fs::remove_file(&tmp_path)) {
            Ok(_) => {}
            Err(e2) => {
                let _ = fs::remove_file(&tmp_path);
                return Err(ServiceError::ExportFailed(format!(
                    "rename from {} to {} failed ({e}); copy fallback also failed: {e2}",
                    tmp_path.display(),
                    output_path.display()
                )));
            }
        }
    }

    Ok(output_path.to_path_buf())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::trace::{EventData, EventType, SourceLocation};
    use chronos_index::builder::IndexBuilder;
    use chronos_query::QueryEngine;
    use chronos_store::SessionMetadata;

    use std::collections::HashMap;

    fn func_event(id: u64, thread: u64, name: &str) -> TraceEvent {
        TraceEvent {
            event_id: id,
            timestamp_ns: id * 1_000,
            thread_id: thread,
            event_type: EventType::FunctionEntry,
            location: SourceLocation {
                function: Some(name.to_string()),
                ..SourceLocation::default()
            },
            data: EventData::Function {
                name: name.to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        }
    }

    fn make_engines_clean(
        session_id: &str,
        events: Vec<TraceEvent>,
    ) -> SessionExportContext<'static> {
        let mut builder = IndexBuilder::new();
        builder.push_all(&events);
        let indices = builder.finalize();
        let engine = QueryEngine::with_indices(events, indices.shadow, indices.temporal)
            .with_causality(indices.causality)
            .with_performance(indices.performance);
        let mut map: HashMap<String, QueryEngine> = HashMap::new();
        map.insert(session_id.to_string(), engine);
        let leaked_mutex: &'static TokioMutex<HashMap<String, QueryEngine>> =
            Box::leak(Box::new(TokioMutex::new(map)));
        SessionExportContext {
            engines: leaked_mutex,
        }
    }

    #[tokio::test]
    async fn export_happy_path_json() {
        let dir = tempdir();
        let path = dir.join("session.json");
        let ctx = make_engines_clean(
            "s1",
            vec![func_event(1, 1, "main"), func_event(2, 1, "worker")],
        );
        let result = ChronosSessionExportService::export(
            "s1",
            Language::Python,
            "app.py".to_string(),
            ExportFormat::Json,
            &path,
            &ctx,
        )
        .await
        .unwrap();

        assert_eq!(result.format, ExportFormat::Json);
        assert_eq!(result.path, path);
        assert!(result.bytes_written > 0);

        let raw = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["schema_version"], "v2-export.1");
        assert_eq!(v["metadata"]["session_id"], "s1");
        assert_eq!(v["events"].as_array().unwrap().len(), 2);
        assert_eq!(v["properties_snapshot"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn export_happy_path_otlp_json() {
        let dir = tempdir();
        let path = dir.join("session.otlp.json");
        let ctx = make_engines_clean(
            "s2",
            vec![func_event(1, 1, "main"), func_event(2, 1, "worker")],
        );
        let result = ChronosSessionExportService::export(
            "s2",
            Language::Python,
            "app.py".to_string(),
            ExportFormat::OtlpJson,
            &path,
            &ctx,
        )
        .await
        .unwrap();

        assert_eq!(result.format, ExportFormat::OtlpJson);

        let raw = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(v["resourceSpans"].is_array());
        let rs0 = &v["resourceSpans"][0];
        assert!(rs0["resource"]["attributes"].is_array());

        // schema.version appears in resource attributes.
        let attrs = rs0["resource"]["attributes"].as_array().unwrap();
        let sv = attrs.iter().find(|a| a["key"] == "schema.version").unwrap();
        assert_eq!(sv["value"]["stringValue"], "v2-export.1");

        let spans = rs0["scopeSpans"][0]["spans"].as_array().unwrap();
        assert_eq!(spans.len(), 2);
    }

    #[tokio::test]
    async fn export_session_not_in_memory() {
        let dir = tempdir();
        let path = dir.join("session.json");
        let ctx = make_engines_clean("other", vec![func_event(1, 1, "main")]);
        let err = ChronosSessionExportService::export(
            "missing",
            Language::Python,
            "x.py".to_string(),
            ExportFormat::Json,
            &path,
            &ctx,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ServiceError::SessionNotInMemory(_)));
    }

    #[tokio::test]
    async fn export_empty_session() {
        let dir = tempdir();
        let path = dir.join("session.json");
        let ctx = make_engines_clean("s1", vec![]);
        let err = ChronosSessionExportService::export(
            "s1",
            Language::Python,
            "x.py".to_string(),
            ExportFormat::Json,
            &path,
            &ctx,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ServiceError::EmptySession(_)));
    }

    #[tokio::test]
    async fn export_properties_snapshot_is_empty_in_m6_05() {
        // Even with multiple events (which m6-05 could in theory project),
        // the snapshot stays empty because the QueryEngine has no
        // property-table accessor yet. This is the documented limitation.
        let dir = tempdir();
        let path = dir.join("session.json");
        let ctx = make_engines_clean(
            "s1",
            vec![
                func_event(1, 1, "main"),
                func_event(2, 1, "worker"),
                func_event(3, 1, "cleanup"),
            ],
        );
        let _ = ChronosSessionExportService::export(
            "s1",
            Language::Python,
            "x.py".to_string(),
            ExportFormat::Json,
            &path,
            &ctx,
        )
        .await
        .unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let snap = v["properties_snapshot"].as_array().unwrap();
        assert_eq!(snap.len(), 0);
    }

    #[tokio::test]
    async fn export_atomic_write_fails_when_parent_missing() {
        let dir = tempdir();
        let bad = dir.join("does/not/exist/session.json");
        let ctx = make_engines_clean("s1", vec![func_event(1, 1, "main")]);
        let err = ChronosSessionExportService::export(
            "s1",
            Language::Python,
            "x.py".to_string(),
            ExportFormat::Json,
            &bad,
            &ctx,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ServiceError::ExportFailed(_)));
    }

    #[tokio::test]
    async fn export_atomic_write_no_tmp_leftover_on_success() {
        let dir = tempdir();
        let path = dir.join("session.json");
        let ctx = make_engines_clean("s1", vec![func_event(1, 1, "main")]);
        ChronosSessionExportService::export(
            "s1",
            Language::Python,
            "x.py".to_string(),
            ExportFormat::Json,
            &path,
            &ctx,
        )
        .await
        .unwrap();

        // Tmp file should have been renamed away -- no `.tmp.<pid>` left.
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .collect();
        assert!(leftovers.is_empty(), "tmp file leaked: {:?}", leftovers);
    }

    #[tokio::test]
    async fn export_schema_version_is_v2_export_1_json() {
        let dir = tempdir();
        let path = dir.join("session.json");
        let ctx = make_engines_clean("s1", vec![func_event(1, 1, "main")]);
        ChronosSessionExportService::export(
            "s1",
            Language::Python,
            "x.py".to_string(),
            ExportFormat::Json,
            &path,
            &ctx,
        )
        .await
        .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["schema_version"], "v2-export.1");
    }

    #[tokio::test]
    async fn export_schema_version_is_v2_export_1_otlp_json() {
        let dir = tempdir();
        let path = dir.join("session.otlp.json");
        let ctx = make_engines_clean("s1", vec![func_event(1, 1, "main")]);
        ChronosSessionExportService::export(
            "s1",
            Language::Python,
            "x.py".to_string(),
            ExportFormat::OtlpJson,
            &path,
            &ctx,
        )
        .await
        .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        // Both at the top-level resource attribute AND in the scope version.
        let attrs = v["resourceSpans"][0]["resource"]["attributes"]
            .as_array()
            .unwrap();
        let sv = attrs.iter().find(|a| a["key"] == "schema.version").unwrap();
        assert_eq!(sv["value"]["stringValue"], "v2-export.1");
    }

    #[tokio::test]
    async fn export_metadata_round_trips() {
        let dir = tempdir();
        let path = dir.join("session.json");
        let ctx = make_engines_clean(
            "s-abc",
            vec![func_event(1, 1, "main"), func_event(2, 1, "main")],
        );
        ChronosSessionExportService::export(
            "s-abc",
            Language::Go,
            "cmd/server.go".to_string(),
            ExportFormat::Json,
            &path,
            &ctx,
        )
        .await
        .unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let m: SessionMetadata = serde_json::from_value(v["metadata"].clone()).unwrap();
        assert_eq!(m.session_id, "s-abc");
        assert_eq!(m.language, "go"); // Language::Go -> "go" via Display
        assert_eq!(m.target, "cmd/server.go");
        assert_eq!(m.event_count, 2);
        assert!(m.duration_ms <= m.created_at || m.created_at > 0);
    }

    #[tokio::test]
    async fn export_events_round_trip() {
        let dir = tempdir();
        let path = dir.join("session.json");
        let ctx = make_engines_clean(
            "s1",
            vec![func_event(1, 1, "main"), func_event(5, 1, "worker")],
        );
        ChronosSessionExportService::export(
            "s1",
            Language::Python,
            "x.py".to_string(),
            ExportFormat::Json,
            &path,
            &ctx,
        )
        .await
        .unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let events: Vec<TraceEvent> = serde_json::from_value(v["events"].clone()).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events.first().unwrap().timestamp_ns, 1_000);
        assert_eq!(events.last().unwrap().timestamp_ns, 5_000);
    }

    #[tokio::test]
    async fn export_bytes_written_positive_for_non_empty_session() {
        let dir = tempdir();
        let path = dir.join("session.json");
        let ctx = make_engines_clean("s1", vec![func_event(1, 1, "main")]);
        let r = ChronosSessionExportService::export(
            "s1",
            Language::Python,
            "x.py".to_string(),
            ExportFormat::Json,
            &path,
            &ctx,
        )
        .await
        .unwrap();
        assert!(r.bytes_written > 0);
        // And it equals the on-disk size.
        let on_disk = std::fs::metadata(&path).unwrap().len();
        assert_eq!(on_disk, r.bytes_written);
    }

    #[tokio::test]
    async fn export_zip_json_is_rejected() {
        let dir = tempdir();
        let path = dir.join("session.zip");
        let ctx = make_engines_clean("s1", vec![func_event(1, 1, "main")]);
        let err = ChronosSessionExportService::export(
            "s1",
            Language::Python,
            "x.py".to_string(),
            ExportFormat::ZipJson,
            &path,
            &ctx,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ServiceError::InvalidExportParameter(_)));
        // And nothing should have been written at the target path.
        assert!(!path.exists());
    }

    // -----------------------------------------------------------------------
    // tiny tempdir helper -- avoids pulling in a tempfile dep just for tests.
    // -----------------------------------------------------------------------
    fn tempdir() -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("chronos-test-{pid}-{nanos}"));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
