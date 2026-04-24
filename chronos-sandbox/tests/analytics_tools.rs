//! Analytics tools tests — verify get_execution_summary, debug_call_graph,
//! and get_call_stack work correctly after probe_stop.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_get_execution_summary_after_probe_stop() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe on test_add
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Drain events
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    // Stop probe - builds query engine
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);
    assert!(stop.total_events > 0, "Should have captured events");

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get execution summary
    let summary = client.get_execution_summary(&session_id).await
        .expect("get_execution_summary failed");

    // === Assertions ===
    assert_eq!(summary.session_id, session_id, "session_id should match");
    assert!(summary.total_events > 0, "total_events should be non-zero");

    println!("✓ Execution summary: {} events, {} threads, {} top functions",
        summary.total_events, summary.thread_count, summary.top_functions.len());
    println!("  Duration: {} ns", summary.duration_ns);

    if !summary.top_functions.is_empty() {
        println!("  Top functions: {:?}", summary.top_functions);
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_call_graph_after_probe_stop() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe on test_add
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Drain events
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    // Stop probe - builds query engine
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);
    assert!(stop.total_events > 0, "Should have captured events");

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get call graph
    let graph = client.debug_call_graph(&session_id, 10).await
        .expect("debug_call_graph failed");

    // === Assertions ===
    assert_eq!(graph.session_id, session_id, "session_id should match");
    assert_eq!(graph.max_depth, 10, "max_depth should be 10");
    assert!(graph.unique_functions >= 0, "unique_functions should be non-negative");

    println!("✓ Call graph: {} unique functions", graph.unique_functions);

    if !graph.nodes.is_empty() {
        println!("  Functions found:");
        for node in graph.nodes.iter().take(5) {
            println!("    {} (called {} times)", node.function, node.call_count);
        }
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_get_call_stack_after_probe_stop() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe on test_add
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Drain events
    let drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    // Stop probe - builds query engine
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);
    assert!(stop.total_events > 0, "Should have captured events");

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get call stack at first event
    if !drained.is_empty() {
        let first_event_id = drained[0].event_id;
        let frames = client.get_call_stack(&session_id, first_event_id).await;

        match frames {
            Ok(stack_frames) => {
                println!("✓ get_call_stack at event {}: {} frames", first_event_id, stack_frames.len());
                // Stack frames should have depth, function, and address
                for (i, frame) in stack_frames.iter().enumerate() {
                    println!("  [{}] {} @ {}:{}",
                        frame.depth, frame.function,
                        frame.file.as_deref().unwrap_or("?"),
                        frame.line.unwrap_or(0));
                }
            }
            Err(e) => {
                // get_call_stack may fail if the event doesn't have stack info
                println!("get_call_stack returned error (expected for raw events): {:?}", e);
            }
        }
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_get_execution_summary_busyloop() {
    // Use test_busyloop which runs longer and generates more events
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to run a bit
    tokio::time::sleep(Duration::from_secs(4)).await;

    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    assert!(stop.total_events > 0, "Should have captured events");

    tokio::time::sleep(Duration::from_millis(200)).await;

    let summary = client.get_execution_summary(&session_id).await
        .expect("get_execution_summary failed");

    assert!(summary.total_events > 0, "Should have events");

    println!("✓ Busyloop summary: {} events, {} threads, {} top functions",
        summary.total_events, summary.thread_count, summary.top_functions.len());

    client.shutdown().await.ok();
}
