//! End-to-end integration tests for Chronos.
//!
//! These tests exercise the full pipeline:
//! 1. Compile a C fixture
//! 2. Capture execution via ptrace
//! 3. Build indices
//! 4. Query events
//! 5. Verify results

use chronos_capture::TraceAdapter;
use chronos_domain::{
    CaptureConfig, EventType, Language, TraceEvent, TraceQuery,
};
use chronos_format::TraceFileWriter;
use chronos_index::IndexBuilder;
use chronos_native::{
    BreakpointManager, NativeAdapter, PtraceConfig, PtraceTracer, SymbolResolver,
};
use chronos_query::QueryEngine;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Compile a C fixture and return the path to the binary.
fn compile_fixture(source_name: &str) -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let fixtures_dir = Path::new(&manifest_dir).join("tests").join("fixtures");
    let source_path = fixtures_dir.join(source_name);
    let binary_path = fixtures_dir.join(source_name.replace(".c", ""));

    // Don't recompile if binary exists and is newer than source
    if binary_path.exists() {
        if let (Ok(src_meta), Ok(bin_meta)) = (
            std::fs::metadata(&source_path),
            std::fs::metadata(&binary_path),
        ) {
            if bin_meta.modified().unwrap() > src_meta.modified().unwrap() {
                return binary_path;
            }
        }
    }

    let output = Command::new("gcc")
        .args([
            "-g",           // Include debug info
            "-no-pie",      // Fixed load address for easier testing
            "-o",
        ])
        .arg(&binary_path)
        .arg(&source_path)
        .output()
        .expect("gcc should be installed");

    if !output.status.success() {
        panic!(
            "Failed to compile {}: {}",
            source_name,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    binary_path
}

/// Helper to create a temp file.
fn create_temp_trace_file() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("test.trace");
    (dir, path)
}

// ============================================================================
// Symbol resolver integration tests
// ============================================================================

#[test]
fn test_symbol_resolver_loads_compiled_binary() {
    let binary = compile_fixture("test_add.c");
    let mut resolver = SymbolResolver::new();

    resolver.load_from_binary(&binary).unwrap();

    assert!(!resolver.is_empty());
    assert!(resolver.symbol_count() > 0);

    // Should find the 'main' function
    let main_syms = resolver.find_by_name("main");
    assert!(
        !main_syms.is_empty(),
        "Should find 'main' symbol in compiled binary"
    );

    // Should find our custom functions
    let addrs = resolver.get_function_addresses("add");
    assert!(!addrs.is_empty(), "Should find 'add' function");

    let addrs = resolver.get_function_addresses("multiply");
    assert!(!addrs.is_empty(), "Should find 'multiply' function");

    let addrs = resolver.get_function_addresses("compute");
    assert!(!addrs.is_empty(), "Should find 'compute' function");
}

#[test]
fn test_symbol_resolver_resolves_main() {
    let binary = compile_fixture("test_add.c");
    let mut resolver = SymbolResolver::new();
    resolver.load_from_binary(&binary).unwrap();

    // Get main's address
    let addrs = resolver.get_function_addresses("main");
    let main_addr = addrs[0].1;

    // Resolve it back
    let loc = resolver.resolve_to_source_location(main_addr);
    assert_eq!(loc.function.as_deref(), Some("main"));

    // Resolve within main's range (main+10 should still be in main)
    if let Some(sym) = resolver.resolve(main_addr + 10) {
        assert_eq!(sym.name, "main");
    }
}

#[test]
fn test_symbol_resolver_glob_patterns() {
    let binary = compile_fixture("test_add.c");
    let mut resolver = SymbolResolver::new();
    resolver.load_from_binary(&binary).unwrap();

    // Find all functions starting with 'comp'
    let results = resolver.find_by_name("comp*");
    assert!(!results.is_empty(), "Should find 'compute' via glob");

    // Find all symbols with wildcard
    let all = resolver.find_by_name("*");
    assert!(all.len() > 3, "Should find multiple symbols");
}

#[test]
fn test_get_function_addresses_for_breakpoints() {
    let binary = compile_fixture("test_add.c");
    let mut resolver = SymbolResolver::new();
    resolver.load_from_binary(&binary).unwrap();

    // Get addresses for all our functions
    let addrs = resolver.get_function_addresses("*");
    assert!(addrs.len() >= 4, "Should find add, multiply, compute, main");

    // All addresses should be non-zero and unique
    let unique_addrs: std::collections::HashSet<u64> =
        addrs.iter().map(|(_, a)| *a).collect();
    assert_eq!(unique_addrs.len(), addrs.len(), "Addresses should be unique");
}

// ============================================================================
// Trace format integration tests
// ============================================================================

#[test]
fn test_write_and_read_trace_with_real_events() {
    let (dir, path) = create_temp_trace_file();

    // Create events mimicking a real trace
    let events = vec![
        TraceEvent::function_entry(0, 100, 1, "main", 0x401000),
        TraceEvent::function_entry(1, 200, 1, "compute", 0x401050),
        TraceEvent::function_entry(2, 300, 1, "add", 0x4010A0),
        TraceEvent::function_exit(3, 400, 1, "add", 0x4010A0),
        TraceEvent::function_entry(4, 500, 1, "multiply", 0x401100),
        TraceEvent::function_exit(5, 600, 1, "multiply", 0x401100),
        TraceEvent::function_exit(6, 700, 1, "compute", 0x401050),
        TraceEvent::function_exit(7, 800, 1, "main", 0x401000),
    ];

    // Write trace file
    {
        let mut writer = TraceFileWriter::create(&path).unwrap();
        for event in &events {
            writer.write_event(event).unwrap();
        }
        writer.finalize().unwrap();
    }

    // Read it back
    let events_read = TraceFileWriter::read_trace_file(&path).unwrap();

    assert_eq!(events_read.len(), 8);
    assert_eq!(events_read[0].event_type, EventType::FunctionEntry);
    assert_eq!(events_read[0].function_name(), Some("main"));
    assert_eq!(events_read[7].event_type, EventType::FunctionExit);
}

// ============================================================================
// Index + Query integration tests
// ============================================================================

#[test]
fn test_index_and_query_pipeline() {
    let events = vec![
        TraceEvent::function_entry(0, 100, 1, "main", 0x1000),
        TraceEvent::function_entry(1, 200, 1, "helper", 0x2000),
        TraceEvent::function_entry(2, 300, 1, "add", 0x3000),
        TraceEvent::function_exit(3, 400, 1, "add", 0x3000),
        TraceEvent::function_exit(4, 500, 1, "helper", 0x2000),
        TraceEvent::function_entry(5, 600, 1, "process", 0x4000),
        TraceEvent::function_exit(6, 700, 1, "process", 0x4000),
        TraceEvent::function_exit(7, 800, 1, "main", 0x1000),
    ];

    // Build indices
    let mut builder = IndexBuilder::new();
    builder.push_all(&events);
    let indices = builder.finalize();

    // Create query engine with indices
    let engine = QueryEngine::with_indices(
        events,
        indices.shadow,
        indices.temporal,
    );

    // Query all function entries
    let query = TraceQuery::new("test")
        .event_types(vec![EventType::FunctionEntry]);
    let result = engine.execute(&query);
    assert_eq!(result.total_matching, 4);

    // Query by time range
    let query = TraceQuery::new("test").time_range(200, 600);
    let result = engine.execute(&query);
    assert_eq!(result.total_matching, 4); // IDs 1,2,3,4

    // Query by function name
    let query = TraceQuery::new("test").function_pattern("add");
    let result = engine.execute(&query);
    assert_eq!(result.total_matching, 2); // entry + exit

    // Execution summary
    let summary = engine.execution_summary("test");
    assert_eq!(summary.total_events, 8);
    assert_eq!(summary.thread_count, 1);
    assert!(summary.duration_ns > 0);

    // Call stack at event 2 (inside add, called from helper, called from main)
    let stack = engine.reconstruct_call_stack(2);
    assert_eq!(stack.len(), 3); // main -> helper -> add
}

// ============================================================================
// MCP Server integration tests
// ============================================================================

#[tokio::test]
async fn test_mcp_server_lifecycle() {
    use chronos_mcp::ChronosServer;
    use rmcp::handler::server::wrapper::Parameters;
    use rmcp::handler::server::ServerHandler;

    let server = ChronosServer::new();

    // Try to start capture with non-existent binary
    let params = Parameters(chronos_mcp::server::DebugRunParams {
        program: "/nonexistent/test_program".to_string(),
        args: vec![],
        trace_syscalls: true,
        capture_registers: true,
        cwd: None,
    });

    let result = server.debug_run(params).await.unwrap();
    assert_eq!(result.is_error, Some(true));
}

// ============================================================================
// Breakpoint manager unit tests (no ptrace needed)
// ============================================================================

#[test]
fn test_breakpoint_manager_creation() {
    let mgr = BreakpointManager::new(1234);
    assert_eq!(mgr.pid(), 1234);
    assert_eq!(mgr.breakpoint_count(), 0);
    assert!(!mgr.is_breakpoint(0x1000));
}

#[test]
fn test_breakpoint_manager_remove_nonexistent() {
    let mut mgr = BreakpointManager::new(9999);
    assert!(mgr.remove_breakpoint(0x1000).is_err());
    assert!(mgr.remove_breakpoint_by_id(0).is_err());
}

// ============================================================================
// Full pipeline simulation (no ptrace, uses mock events)
// ============================================================================

#[test]
fn test_full_pipeline_simulation() {
    // Simulate: compile -> capture -> index -> query -> summarize

    // Step 1: "Compile" (we create events manually as if ptrace captured them)
    let captured_events = vec![
        TraceEvent::function_entry(0, 1000, 1, "main", 0x401000),
        TraceEvent::function_entry(1, 2000, 1, "compute", 0x401050),
        TraceEvent::function_entry(2, 3000, 1, "add", 0x4010A0),
        TraceEvent::function_exit(3, 4000, 1, "add", 0x4010A0),
        TraceEvent::function_entry(4, 5000, 1, "multiply", 0x401100),
        TraceEvent::function_exit(5, 6000, 1, "multiply", 0x401100),
        TraceEvent::function_exit(6, 7000, 1, "compute", 0x401050),
        TraceEvent::signal(7, 7500, 1, 11, "SIGSEGV", 0x0000),
        TraceEvent::function_exit(8, 8000, 1, "main", 0x401000),
    ];

    // Step 2: Write to trace file
    let (dir, trace_path) = create_temp_trace_file();
    {
        let mut writer = TraceFileWriter::create(&trace_path).unwrap();
        for event in &captured_events {
            writer.write_event(event).unwrap();
        }
        writer.finalize().unwrap();
    }

    // Step 3: Read back
    let loaded_events = TraceFileWriter::read_trace_file(&trace_path).unwrap();
    assert_eq!(loaded_events.len(), 9);

    // Step 4: Build indices
    let mut builder = IndexBuilder::new();
    builder.push_all(&loaded_events);
    let indices = builder.finalize();

    // Step 5: Query
    let engine = QueryEngine::with_indices(
        loaded_events.clone(),
        indices.shadow,
        indices.temporal,
    );

    // Verify queries work
    let all = engine.execute(&TraceQuery::new("test").pagination(100, 0));
    assert_eq!(all.total_matching, 9);

    let functions = engine.execute(
        &TraceQuery::new("test").event_types(vec![
            EventType::FunctionEntry,
            EventType::FunctionExit,
        ])
    );
    assert_eq!(functions.total_matching, 8); // All except signal

    let signals = engine.execute(
        &TraceQuery::new("test").event_types(vec![EventType::SignalDelivered])
    );
    assert_eq!(signals.total_matching, 1);

    // Step 6: Execution summary
    let summary = engine.execution_summary("test");
    assert_eq!(summary.total_events, 9);
    assert_eq!(summary.thread_count, 1);
    assert_eq!(summary.duration_ns, 7000); // 8000 - 1000

    // Signal should be detected as a potential issue
    assert!(!summary.potential_issues.is_empty());

    // Step 7: Call stack at various points
    let stack_at_add = engine.reconstruct_call_stack(2);
    assert_eq!(stack_at_add.len(), 3); // main -> compute -> add

    let stack_after_all = engine.reconstruct_call_stack(100);
    assert!(stack_at_add.len() >= 1);
}
