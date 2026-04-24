//! Thread tracking test — verify list_threads with multi-threaded C program.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_thread_tracking() {
    let fixture = McpSession::fixture_path("test_threads")
        .expect("test_threads fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe on multi-threaded program
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for threads to run
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Stop probe - builds query engine
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // List threads
    let threads = client.list_threads(&session_id).await
        .expect("list_threads failed");

    println!("Detected {} threads: {:?}", threads.len(), threads);

    // === Assertions ===
    assert!(!threads.is_empty(), "Should detect at least 1 thread");

    // test_threads creates 3 worker threads + main thread = 4 threads
    // pthread_create uses clone() which is tracked via PTRACE_O_TRACECLONE
    assert!(threads.len() >= 2, "Should detect main + worker threads, got {}", threads.len());

    println!("✓ Thread tracking works: {} threads detected", threads.len());

    client.shutdown().await.ok();
}
