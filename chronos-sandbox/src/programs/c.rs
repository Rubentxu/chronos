//! C program fixtures for MCP sandbox testing.

macro_rules! fixture_path {
    ($name:literal) => {
        concat!(env!("OUT_DIR"), "/", $name)
    };
}

/// Path to the compiled `test_add` binary.
pub fn test_add() -> &'static str {
    fixture_path!("test_add")
}

/// Path to the compiled `test_busyloop` binary.
pub fn test_busyloop() -> &'static str {
    fixture_path!("test_busyloop")
}

/// Path to the compiled `test_segfault` binary.
pub fn test_segfault() -> &'static str {
    fixture_path!("test_segfault")
}

/// Path to the compiled `test_threads` binary.
pub fn test_threads() -> &'static str {
    fixture_path!("test_threads")
}

/// Path to the compiled `test_clone` binary.
pub fn test_clone() -> &'static str {
    fixture_path!("test_clone")
}

/// Path to the compiled `test_crash_thread` binary.
pub fn test_crash_thread() -> &'static str {
    fixture_path!("test_crash_thread")
}

/// Path to the compiled `test_fork` binary.
pub fn test_fork() -> &'static str {
    fixture_path!("test_fork")
}

/// Path to the compiled `test_many_threads` binary.
pub fn test_many_threads() -> &'static str {
    fixture_path!("test_many_threads")
}
