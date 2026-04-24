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
    ];

    for prog in &programs {
        let src = format!("programs/c/{}.c", prog);
        let out = format!("{}/{}", out_dir, prog);

        Command::new("gcc")
            .args(["-g", "-O0", "-pthread", &src, "-o", &out])
            .status()
            .expect(&format!("Failed to compile {}", prog));

        // Tell cargo to rerun if source changes
        println!("cargo:rerun-if-changed={}", src);
    }
}
