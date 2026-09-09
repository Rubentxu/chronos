//! Build script for chronos-mcp integration tests.
//!
//! Compiles the `test_add` C fixture into the OUT_DIR so that
//! `chronos-mcp/tests/debug_read_tools.rs` can find it via `McpSession::fixture_path`.

fn main() {
    // Tell cargo to rerun this script if the source changes.
    println!("cargo:rerun-if-changed=programs/c/test_add.c");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let src = "programs/c/test_add.c";
    let dst = format!("{}/test_add", out_dir);

    let status = std::process::Command::new("gcc")
        .args(["-g", "-O0", "-pthread", src, "-o", &dst])
        .status()
        .expect("failed to compile test_add fixture");

    if !status.success() {
        panic!(
            "gcc failed to compile {}: {}",
            src,
            status.code().unwrap_or(-1)
        );
    }

    // Propagate the fixture path to tests via env var.
    println!("cargo:rustc-env=TEST_ADD_FIXTURE={}", dst);
}
