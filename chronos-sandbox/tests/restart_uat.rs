//! Real two-process restart UAT for clean sealed lifecycle persistence.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::McpSession;
use std::path::PathBuf;

fn unique_root() -> PathBuf {
    std::env::temp_dir().join(format!("chronos-restart-uat-{}", std::process::id()))
}

#[tokio::test]
async fn r3_clean_session_stop_seals_and_restart_bootstraps_it() {
    let root = unique_root();
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::env::set_var("CHRONOS_EXECUTION_LOG_DIR", &root);

    let db = root.join("sessions.redb");
    let fixture = McpSession::fixture_path("test_add").expect("fixture");

    let mut first = McpTestClient::start_with_db_path(db.clone()).await.unwrap();
    let started = first
        .session_start_spawn(fixture.to_str().unwrap(), vec![])
        .await
        .unwrap();
    let stopped = first
        .session_stop(&started.session_id, true, true)
        .await
        .unwrap();
    assert!(
        stopped.sealed_at.is_some(),
        "clean stop must report sealed_at"
    );
    first.shutdown().await.unwrap();

    let mut second = McpTestClient::start_with_db_path(db).await.unwrap();
    let loaded = second
        .session_start_load(&started.session_id)
        .await
        .unwrap();
    assert_eq!(loaded.session_id, started.session_id);
    assert_eq!(loaded.capability_snapshot["tail_sealed"], true);
    second.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn r4_delete_session_is_absent_after_restart() {
    let root = unique_root().join("r4");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::env::set_var("CHRONOS_EXECUTION_LOG_DIR", &root);
    let db = root.join("sessions.redb");
    let fixture = McpSession::fixture_path("test_add").expect("fixture");

    let mut first = McpTestClient::start_with_db_path(db.clone()).await.unwrap();
    let started = first
        .session_start_spawn(fixture.to_str().unwrap(), vec![])
        .await
        .unwrap();
    first
        .session_stop(&started.session_id, true, true)
        .await
        .unwrap();
    let deleted = first
        .call_tool(
            "delete_session",
            serde_json::json!({"session_id": started.session_id.clone()}),
        )
        .await
        .unwrap();
    assert_eq!(deleted["status"], "deleted");
    eprintln!("delete response: {deleted:#}");
    let paths = deleted
        .get("paths_removed")
        .or_else(|| deleted.get("result").and_then(|v| v.get("paths_removed")))
        .and_then(serde_json::Value::as_array)
        .expect("delete response must expose paths_removed");
    assert!(!paths.is_empty());
    first.shutdown().await.unwrap();

    let mut second = McpTestClient::start_with_db_path(db).await.unwrap();
    let missing = second.session_start_load(&started.session_id).await;
    assert!(
        missing.is_err(),
        "deleted session must not reappear after restart"
    );
    second.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(root);
}
