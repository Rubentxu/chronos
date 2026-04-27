//! WASM test fixtures
use std::path::PathBuf;

/// Return the path to the compiled add.wasm fixture
pub fn add_wasm_path() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("wasm");
    dir.join("add.wasm")
}

/// Ensure the WASM binary exists (create from bytes if needed)
pub fn ensure_add_wasm() -> PathBuf {
    let path = add_wasm_path();
    if !path.exists() {
        std::fs::write(&path, create_minimal_wasm_bytes())
            .expect("Failed to write add.wasm fixture");
    }
    path
}

/// Minimal WASM binary bytes for add(a: i32, b: i32) -> i32
///
/// This is a valid WebAssembly module that exports a single `add` function
/// which takes two i32 parameters and returns their sum.
///
/// (module
///   (func (export "add") (param i32 i32) (result i32)
///     local.get 0 local.get 1 i32.add))
pub fn create_minimal_wasm_bytes() -> Vec<u8> {
    vec![
        0x00, 0x61, 0x73, 0x6d, // magic: \0asm
        0x01, 0x00, 0x00, 0x00, // version: 1

        // Type section (1 type): (func (param i32 i32) (result i32))
        0x01, 0x07, 0x01, // section id, size, count
        0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f, // functype

        // Function section (1 function): type index 0
        0x03, 0x02, 0x01, 0x00,

        // Export section (1 export): "add" -> func index 0
        0x07, 0x07, 0x01, // section id, size, count
        0x03, 0x61, 0x64, 0x64, // "add"
        0x00, 0x00, // export kind: function, index 0

        // Code section (1 body)
        0x0a, 0x09, 0x01, // section id, size, count
        0x07, 0x00, // body size, local count
        0x20, 0x00, // local.get 0
        0x20, 0x01, // local.get 1
        0x6a, // i32.add
        0x0b, // end
    ]
}
