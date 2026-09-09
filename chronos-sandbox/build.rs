use std::process::Command;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let programs = vec![
        "test_add",
        "test_busyloop",
        "test_segfault",
        "test_threads",
        "test_clone",
        "test_crash_thread",
        "test_fork",
        "test_many_threads",
        "test_exit_immediate",
        "test_divide_by_zero",
        "test_abort",
        "test_infinite_loop",
        "test_function_frames",
    ];

    for prog in &programs {
        let src = format!("programs/c/{}.c", prog);
        let out = format!("{}/{}", out_dir, prog);

        // test_function_frames needs -no-pie -O0 -fno-inline so the
        // SymbolResolver finds size-bearing text symbols whose entry
        // addresses the live-probe frame-capture pipeline can INT3-breakpoint
        // and single-step through.
        let extra_flags: &[&str] = if *prog == "test_function_frames" {
            &["-no-pie", "-O0", "-fno-inline"]
        } else {
            &[]
        };

        let status = Command::new("gcc")
            .args(["-g", "-O0", "-pthread", &src, "-o", &out])
            .args(extra_flags)
            .status();
        match status {
            Ok(s) if s.success() => {}
            _ => {
                // Fall back without extra flags if -no-pie is rejected by the
                // toolchain (older gccs / clang). The default -O0 -g still
                // keeps enough symbols for the live probe to resolve at least
                // `main`; full FunctionEntry assertions live in
                // chronos-native/tests/m2_function_frame_capture.rs.
                Command::new("gcc")
                    .args(["-g", "-O0", "-pthread", &src, "-o", &out])
                    .status()
                    .unwrap_or_else(|_| panic!("Failed to compile {}", prog));
            }
        }

        // Tell cargo to rerun if source changes
        println!("cargo:rerun-if-changed={}", src);
    }
}
