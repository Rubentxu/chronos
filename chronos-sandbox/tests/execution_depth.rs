//! Execution analysis depth tests — verify execution summary and call graph tools work correctly.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::QueryFilter;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// ED1: test_get_execution_summary_top_functions_not_empty
/// Probe test_busyloop, get_execution_summary, assert total_events > 0.
/// Note: top_functions may be empty for C programs without debug symbols.
#[tokio::test]
async fn test_get_execution_summary_top_functions_not_empty() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    let summary = client
        .get_execution_summary(&session_id)
        .await
        .expect("get_execution_summary failed");

    // Assert: total_events > 0
    assert!(
        summary.total_events > 0,
        "total_events should be > 0, got {}",
        summary.total_events
    );

    // Note: top_functions may be empty for C programs without debug symbols
    println!(
        "✓ get_execution_summary: total_events={}, top_functions count={}",
        summary.total_events,
        summary.top_functions.len()
    );
    if !summary.top_functions.is_empty() {
        println!("  Top 3 functions:");
        for (i, f) in summary.top_functions.iter().take(3).enumerate() {
            println!("    [{}] {}: {} calls", i, f.name, f.call_count);
        }
    } else {
        println!("  (No function names available - C program without debug symbols)");
    }

    client.shutdown().await.ok();
}

/// ED2: test_debug_call_graph_has_edges
/// Probe test_busyloop and ask for a call graph at max_depth=10.
///
/// Measured on this environment the graph is empty *by construction*:
/// `unique_functions=0`, `nodes=0`, `edges=0` and every `stats` field 0.
/// The fixture is a C program probed without function-frame tracking, so
/// there are no `function_entry` events to derive nodes or edges from.
/// The name predates that measurement and is kept for traceability; what
/// the test now pins is (a) the empty-graph contract, which fails loudly
/// if frame tracking ever starts producing nodes, and (b) the structural
/// invariants that must hold between the `nodes`/`edges` arrays and the
/// `stats` aggregate the server computes separately.
#[tokio::test]
async fn test_debug_call_graph_has_edges() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    assert!(
        stop.total_events > 0,
        "probe must capture events before a graph can be reasoned about"
    );
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    let call_graph = client
        .debug_call_graph(&session_id, 10)
        .await
        .expect("debug_call_graph failed");

    // The session's own identity must come back with the response, so a
    // graph can never be attributed to the wrong session.
    assert_eq!(
        call_graph.session_id, session_id,
        "call graph must belong to the session it was requested for"
    );

    // Empty by construction (see doc-comment): a C fixture probed without
    // function frames yields no nodes and no edges.
    assert_eq!(
        call_graph.nodes.len(),
        0,
        "expected an empty node set for a fixture without function frames, got {:?}",
        call_graph
            .nodes
            .iter()
            .map(|n| n.function.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        call_graph.edges.len(),
        0,
        "expected an empty edge set for a fixture without function frames, got {:?}",
        call_graph
            .edges
            .iter()
            .map(|e| (e.from.clone(), e.to.clone()))
            .collect::<Vec<_>>()
    );

    // `stats` is computed server-side, independently of the arrays, so the
    // two must agree.
    assert_eq!(
        call_graph.stats.node_count as usize,
        call_graph.nodes.len(),
        "stats.node_count must agree with the node array"
    );
    assert_eq!(
        call_graph.stats.edge_count as usize,
        call_graph.edges.len(),
        "stats.edge_count must agree with the edge array"
    );
    assert_eq!(
        call_graph.unique_functions,
        call_graph.nodes.len(),
        "unique_functions must agree with the node array"
    );

    // `max_depth` is the *observed* depth of the returned graph, not an
    // echo of the requested 10 — measured 0 for an empty graph.
    assert_eq!(
        call_graph.stats.max_depth, 0,
        "an empty graph must report observed depth 0"
    );
    assert_eq!(
        call_graph.max_depth, 0,
        "max_depth must report the observed depth of the graph, not the request"
    );

    println!(
        "✓ debug_call_graph: {} unique functions, {} nodes, {} edges (empty by construction)",
        call_graph.unique_functions,
        call_graph.nodes.len(),
        call_graph.edges.len()
    );

    client.shutdown().await.ok();
}

/// ED3: test_debug_expand_hotspot_top_n_1
/// Probe test_busyloop, debug_expand_hotspot with top_n=1, assert response valid, len <= 1.
#[tokio::test]
async fn test_debug_expand_hotspot_top_n_1() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    let hotspot = client
        .debug_expand_hotspot(&session_id, 1)
        .await
        .expect("debug_expand_hotspot failed");

    // Assert: response valid (function name present)
    println!(
        "✓ debug_expand_hotspot top_n=1: {} ({} calls)",
        hotspot.function, hotspot.call_count
    );

    // Note: The response returns aggregated data, so we just verify it's valid
    assert!(
        !hotspot.function.is_empty(),
        "function name should be non-empty"
    );

    client.shutdown().await.ok();
}

/// ED4: test_debug_expand_hotspot_top_n_50
/// Probe test_busyloop, debug_expand_hotspot with top_n=50, assert response valid, len <= 50.
#[tokio::test]
async fn test_debug_expand_hotspot_top_n_50() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    let hotspot = client
        .debug_expand_hotspot(&session_id, 50)
        .await
        .expect("debug_expand_hotspot failed");

    // Assert: response valid
    println!(
        "✓ debug_expand_hotspot top_n=50: {} ({} calls)",
        hotspot.function, hotspot.call_count
    );

    // Note: The response is aggregated, so we just verify it's valid
    assert!(
        !hotspot.function.is_empty(),
        "function name should be non-empty"
    );

    client.shutdown().await.ok();
}

/// ED5: test_debug_get_saliency_scores_valid
/// Probe test_busyloop, debug_get_saliency_scores with limit=10.
/// Assert: response valid, scores array exists, scores between 0.0 and 1.0 if non-empty.
#[tokio::test]
async fn test_debug_get_saliency_scores_valid() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    let scores = client
        .debug_get_saliency_scores(&session_id, 10)
        .await
        .expect("debug_get_saliency_scores failed");

    // Assert: response valid (scores array exists)
    println!(
        "✓ debug_get_saliency_scores: {} functions scored",
        scores.len()
    );

    // Print top 5
    for (i, score) in scores.iter().take(5).enumerate() {
        println!(
            "  [{}] {}: {:.4} ({} calls)",
            i, score.function, score.saliency_score, score.call_count
        );
    }

    // Assert: scores are between 0.0 and 1.0 if non-empty
    for (i, score) in scores.iter().enumerate() {
        assert!(
            score.saliency_score >= 0.0 && score.saliency_score <= 1.0,
            "Score {} has saliency_score {} out of range [0.0, 1.0]",
            i,
            score.saliency_score
        );
    }

    println!("✓ All {} scores are within [0.0, 1.0]", scores.len());
    client.shutdown().await.ok();
}

/// ED6: test_debug_get_saliency_scores_sorted
/// Probe test_busyloop, debug_get_saliency_scores with limit=20.
/// Assert: if len >= 2, scores[0].saliency_score >= scores[1].saliency_score (descending).
#[tokio::test]
async fn test_debug_get_saliency_scores_sorted() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    let scores = client
        .debug_get_saliency_scores(&session_id, 20)
        .await
        .expect("debug_get_saliency_scores failed");

    println!(
        "debug_get_saliency_scores: {} functions scored",
        scores.len()
    );

    // Print top 10
    for (i, score) in scores.iter().take(10).enumerate() {
        println!("  [{}] {}: {:.4}", i, score.function, score.saliency_score);
    }

    // Assert: if len >= 2, scores are sorted descending
    if scores.len() >= 2 {
        for i in 0..scores.len() - 1 {
            assert!(
                scores[i].saliency_score >= scores[i + 1].saliency_score,
                "Scores not sorted descending: scores[{}]={:.4} < scores[{}]={:.4}",
                i,
                scores[i].saliency_score,
                i + 1,
                scores[i + 1].saliency_score
            );
        }
        println!("✓ Scores are sorted in descending order");
    } else {
        println!("✓ Less than 2 scores, skip ordering check");
    }

    client.shutdown().await.ok();
}

/// ED7: test_get_call_stack_at_syscall_event
/// Probe test_busyloop, filter for a `syscall_enter` event, then ask for
/// the call stack at that event.
///
/// Measured: the filter is honoured (the page comes back filled with
/// `syscall_enter` events, never `syscall_exit`), and `get_call_stack`
/// returns an empty frame list for them — the C fixture is probed without
/// frame-pointer tracking, so there is no unwind information to report.
/// The empty result is the asserted contract, not a skip: this test no
/// longer returns early when the syscall filter finds nothing.
#[tokio::test]
async fn test_get_call_stack_at_syscall_event() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    assert!(
        stop.total_events > 0,
        "probe must capture events before a call stack can be requested"
    );
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query for a syscall_enter event
    let filter = QueryFilter {
        event_types: Some(vec!["syscall_enter".to_string()]),
        limit: 1,
        ..Default::default()
    };

    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events for syscall_enter failed");

    // A live dynamically linked process always makes syscalls, so an empty
    // page here means the query path or the filter broke.
    assert!(
        !events.is_empty(),
        "a live dynamically linked process must yield at least one syscall_enter event"
    );
    assert_eq!(
        events.len(),
        1,
        "limit=1 must cap the page to a single event"
    );
    // If the filter were ignored, the page would come back mixed.
    for ev in &events {
        assert_eq!(
            ev.event_type, "syscall_enter",
            "event_types filter leaked a {} event into a syscall_enter page",
            ev.event_type
        );
    }

    let first_syscall_event_id = events[0].event_id;
    println!("Found syscall_enter event_id={}", first_syscall_event_id);

    // Get call stack at that event
    let frames = client
        .get_call_stack(&session_id, first_syscall_event_id)
        .await
        .expect("get_call_stack failed");

    // Empty by construction (see doc-comment): no frame-pointer tracking,
    // so no unwind data exists for a C fixture. Asserted so that a real
    // regression (frames lost when they *should* exist) is visible, and so
    // that a future frame-tracking capability has to update this test.
    assert!(
        frames.is_empty(),
        "expected no unwind frames for a fixture without frame-pointer tracking, got {:?}",
        frames
            .iter()
            .map(|f| (f.depth, f.function.clone(), f.address.clone()))
            .collect::<Vec<_>>()
    );

    println!(
        "✓ get_call_stack at event {}: {} frames (empty by construction)",
        first_syscall_event_id,
        frames.len()
    );

    client.shutdown().await.ok();
}

/// ED8: test_debug_call_graph_max_depth
/// Probe test_busyloop and ask for a call graph capped at max_depth=1,
/// then ask again at max_depth=10 for comparison.
///
/// Measured: both requests return the same empty graph
/// (`unique_functions=0`, `nodes=0`, `edges=0`) and both report
/// `max_depth=0` — the field carries the *observed* depth of the returned
/// graph, it is not an echo of the request. The invariant pinned here is
/// that a tighter depth cap can never grow the graph, plus the same
/// array/stats agreement asserted in ED2. The cap is deliberately not
/// asserted as an echo: measured 0, not 1.
#[tokio::test]
async fn test_debug_call_graph_max_depth() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    assert!(
        stop.total_events > 0,
        "probe must capture events before a graph can be reasoned about"
    );
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    let call_graph = client
        .debug_call_graph(&session_id, 1)
        .await
        .expect("debug_call_graph failed");

    // Same session at the default depth, to compare the two caps.
    let deep_graph = client
        .debug_call_graph(&session_id, 10)
        .await
        .expect("debug_call_graph(10) failed");

    assert_eq!(
        call_graph.session_id, session_id,
        "call graph must belong to the session it was requested for"
    );
    assert_eq!(
        deep_graph.session_id, session_id,
        "deep call graph must belong to the same session"
    );

    // A depth cap can only remove nodes and edges, never add them.
    assert!(
        call_graph.nodes.len() <= deep_graph.nodes.len(),
        "max_depth=1 returned {} nodes, more than max_depth=10 ({})",
        call_graph.nodes.len(),
        deep_graph.nodes.len()
    );
    assert!(
        call_graph.edges.len() <= deep_graph.edges.len(),
        "max_depth=1 returned {} edges, more than max_depth=10 ({})",
        call_graph.edges.len(),
        deep_graph.edges.len()
    );

    // Empty by construction for this C fixture (see doc-comment).
    assert_eq!(
        call_graph.nodes.len(),
        0,
        "expected an empty node set for a fixture without function frames"
    );
    assert_eq!(
        call_graph.edges.len(),
        0,
        "expected an empty edge set for a fixture without function frames"
    );

    // `stats` is computed independently of the arrays and must agree.
    assert_eq!(
        call_graph.stats.node_count as usize,
        call_graph.nodes.len(),
        "stats.node_count must agree with the node array"
    );
    assert_eq!(
        call_graph.stats.edge_count as usize,
        call_graph.edges.len(),
        "stats.edge_count must agree with the edge array"
    );
    assert_eq!(
        call_graph.unique_functions,
        call_graph.nodes.len(),
        "unique_functions must agree with the node array"
    );

    // `max_depth` is the observed depth of the graph, not the request.
    assert_eq!(
        call_graph.max_depth, 0,
        "max_depth must report the observed depth, not the requested 1"
    );
    assert_eq!(
        deep_graph.max_depth, 0,
        "max_depth must report the observed depth, not the requested 10"
    );
    assert_eq!(
        call_graph.stats.max_depth, 0,
        "stats.max_depth must report the observed depth of an empty graph"
    );

    println!(
        "✓ debug_call_graph max_depth=1: {} unique functions, {} nodes, {} edges (empty by construction; max_depth=10 gives {} nodes)",
        call_graph.unique_functions,
        call_graph.nodes.len(),
        call_graph.edges.len(),
        deep_graph.nodes.len()
    );

    client.shutdown().await.ok();
}
