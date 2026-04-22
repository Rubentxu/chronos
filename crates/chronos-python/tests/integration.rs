//! Integration tests for chronos-python DAP adapter.
//!
//! These tests verify Python/debugpy integration. Tests that require
//! a real debugpy instance are gated with
//! `#[cfg_attr(not(feature = "integration"), ignore)]` and require
//! `cargo test --features integration -- --ignored` to run.

use std::process::Command;

/// Lightweight test that verifies DAP Content-Length framing without requiring Python.
/// This test runs always and does not need the `integration` feature.
#[test]
fn test_dap_message_framing() {
    // Test Content-Length framing: encode "hello" → "Content-Length: 5\r\n\r\nhello"
    use chronos_python::client::DapClient;

    // Test parsing Content-Length header
    let header = "Content-Length: 5\r\n\r\n";
    let len = DapClient::parse_content_length(header).unwrap();
    assert_eq!(len, 5);

    // Test with extra whitespace
    let header2 = "Content-Length:    42\r\n\r\n";
    let len2 = DapClient::parse_content_length(header2).unwrap();
    assert_eq!(len2, 42);

    // Test that missing Content-Length returns error
    let header_bad = "Content-Type: application/json\r\n\r\n";
    let result = DapClient::parse_content_length(header_bad);
    assert!(result.is_err());
}

#[test]
#[cfg_attr(not(feature = "integration"), ignore)]
fn test_python_debugpy_capture() {
    // Check python3 is on PATH
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("python3 not found on PATH — skipping integration test");
        return;
    }

    // Check debugpy is installed: python3 -m debugpy --version
    let debugpy_check = Command::new("python3")
        .args(&["-m", "debugpy", "--version"])
        .output();

    if debugpy_check.is_err() {
        eprintln!("debugpy not found — skipping integration test");
        return;
    }

    if !debugpy_check.unwrap().status.success() {
        eprintln!("debugpy check failed — skipping integration test");
        return;
    }

    // Create a simple Python script to debug
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let script_path = temp_dir.path().join("hello.py");
    let script_content = r#"
def foo():
    x = 42
    return x

result = foo()
print(f"Result: {result}")
"#;
    std::fs::write(&script_path, script_content)
        .expect("Failed to write hello.py");

    // Spawn debugpy server
    // debugpy --listen 127.0.0.1:5679 --wait-for-client <script>
    let mut debugpy_child = std::process::Command::new("python3")
        .args(&[
            "-m",
            "debugpy",
            "--listen",
            "127.0.0.1:5679",
            "--wait-for-client",
            script_path.to_str().unwrap(),
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn debugpy");

    // Wait for debugpy to start listening (give it a moment)
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // Try to connect to debugpy
    use chronos_python::client::DapClient;

    let mut client = match DapClient::connect("127.0.0.1:5679") {
        Ok(c) => c,
        Err(e) => {
            // Kill the debugpy process before returning
            let _ = debugpy_child.kill();
            eprintln!("Failed to connect to debugpy: {}", e);
            panic!("DapClient::connect failed: {}", e);
        }
    };

    // Initialize the debug session
    let init_result = client.initialize(0);
    if let Err(e) = init_result {
        let _ = debugpy_child.kill();
        panic!("Failed to initialize DAP session: {}", e);
    }

    // Wait for stopped event (with timeout)
    let timeout_secs = 5;
    let start = std::time::Instant::now();
    let mut stopped_event_received = false;

    loop {
        if start.elapsed().as_secs() > timeout_secs {
            let _ = debugpy_child.kill();
            panic!("Timeout waiting for stopped event from debugpy");
        }

        match client.next_event() {
            Ok(Some(event)) => {
                if event.event == "stopped" {
                    stopped_event_received = true;
                    break;
                }
            }
            Ok(None) => {
                // Session terminated
                break;
            }
            Err(e) => {
                let _ = debugpy_child.kill();
                panic!("Error reading event: {}", e);
            }
        }
    }

    // Assert we received a stopped event
    assert!(
        stopped_event_received,
        "Expected a 'stopped' event from debugpy"
    );

    // Clean up: kill debugpy process
    let _ = debugpy_child.kill();
}
