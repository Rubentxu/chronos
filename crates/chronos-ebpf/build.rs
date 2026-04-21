//! Build script for chronos-ebpf.
//!
//! This script compiles the eBPF uprobe program (uprobe.bpf.c) to uprobe.bpf.o
//! using clang with the BPF target. If clang is not available or compilation
//! fails, a minimal stub object file is created that allows the crate to
//! compile (but not execute the BPF program).

use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let ebpf_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("ebpf");
    let bpf_o_path = ebpf_dir.join("uprobe.bpf.o");

    // Try to compile the BPF program
    let compile_success = compile_bpf_program(&ebpf_dir, &bpf_o_path);

    if !compile_success {
        eprintln!(
            "warning: Failed to compile eBPF program, creating stub object file"
        );
        // Create a minimal stub object file using gcc
        create_stub_object(&bpf_o_path);
    }

    // Tell Cargo to rerun this build script if the BPF source changes
    println!(
        "cargo:rerun-if-changed={}",
        ebpf_dir.join("uprobe.bpf.c").display()
    );
}

/// Create a minimal stub object file that will cause a clear error at runtime
/// when aya tries to load it.
fn create_stub_object(out_path: &Path) {
    // Create a minimal C file that will compile to an empty object
    let temp_c = Path::new("/tmp/chronos_ebpf_stub.c");
    let temp_o = Path::new("/tmp/chronos_ebpf_stub.o");

    // Write minimal C code
    if let Err(e) = std::fs::write(temp_c, "void __stub() {}") {
        eprintln!("Failed to write stub C file: {}", e);
        return;
    }

    // Compile with gcc to create a minimal x86-64 object (not BPF)
    // This will fail to load as BPF but allows compilation
    let output = Command::new("gcc")
        .args([
            "-c",
            temp_c.to_str().unwrap_or("/tmp/chronos_ebpf_stub.c"),
            "-o",
            temp_o.to_str().unwrap_or("/tmp/chronos_ebpf_stub.o"),
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            // Copy the stub to the output location
            if let Err(e) = std::fs::copy(temp_o, out_path) {
                eprintln!("Failed to copy stub object: {}", e);
            } else {
                eprintln!("Created stub object at {}", out_path.display());
            }
        }
        Ok(out) => {
            eprintln!(
                "gcc stub compilation failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Err(e) => {
            eprintln!("Failed to run gcc: {}", e);
        }
    }

    // Clean up temp files
    let _ = std::fs::remove_file(temp_c);
    let _ = std::fs::remove_file(temp_o);
}

fn compile_bpf_program(ebpf_dir: &Path, out_path: &Path) -> bool {
    let src_path = ebpf_dir.join("uprobe.bpf.c");

    // Find clang
    let clang = match find_clang() {
        Some(c) => c,
        None => {
            eprintln!("clang not found, cannot compile eBPF program");
            return false;
        }
    };

    // Try to find BPF headers
    let bpf_include = find_bpf_include_path();

    // Build the clang command
    let mut cmd = Command::new(&clang);
    cmd.args(["-target", "bpf", "-O2", "-g", "-c"]);

    // Add BPF include path if found
    if let Some(ref include_path) = bpf_include {
        cmd.arg("-I").arg(include_path);
    }

    // Add kernel UAPI headers
    if let Ok(kernel_version) = std::fs::read_to_string("/proc/version") {
        // Extract kernel version string like "5.15.0-91-generic"
        if let Some(version_str) = kernel_version.split_whitespace().nth(2) {
            let base_path = format!("/usr/src/linux-headers-{}/include", version_str);
            if Path::new(&base_path).exists() {
                cmd.arg("-I").arg(format!("{}/uapi", base_path));
            }
        }
    }

    // Add standard include paths for BPF headers
    cmd.arg("-I").arg("/usr/include");

    cmd.arg(src_path.as_os_str());
    cmd.arg("-o").arg(out_path.as_os_str());

    println!("Compiling eBPF program: {:?}", cmd);

    match cmd.output() {
        Ok(output) => {
            if output.status.success() {
                println!("eBPF program compiled successfully");
                true
            } else {
                eprintln!(
                    "eBPF compilation failed:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                false
            }
        }
        Err(e) => {
            eprintln!("Failed to run clang: {}", e);
            false
        }
    }
}

fn find_clang() -> Option<String> {
    // Check for clang
    if Command::new("clang").arg("--version").output().is_ok() {
        return Some("clang".to_string());
    }
    // Check for clang-18 or other versions
    for version in &["clang-18", "clang-17", "clang-16", "clang-15"] {
        if Command::new(*version).arg("--version").output().is_ok() {
            return Some(version.to_string());
        }
    }
    None
}

fn find_bpf_include_path() -> Option<String> {
    // Look for BPF headers in common locations
    let candidates = vec![
        "/usr/include/bpf",
        "/usr/local/include/bpf",
        "/usr/lib/bpf",
    ];

    for candidate in candidates {
        let path = Path::new(candidate);
        if path.exists() {
            // Verify it has bpf_helpers.h
            if path.join("bpf_helpers.h").exists() {
                return Some(candidate.to_string());
            }
        }
    }

    // Try to find via pkg-config
    if let Ok(pkg_config_output) = Command::new("pkg-config")
        .args(["--cflags", "libbpf"])
        .output()
    {
        let output_str = String::from_utf8_lossy(&pkg_config_output.stdout);
        if let Some(path) = output_str.strip_prefix("-I") {
            let path = path.trim().to_string();
            if Path::new(&path).join("bpf_helpers.h").exists() {
                return Some(path);
            }
        }
    }

    None
}
