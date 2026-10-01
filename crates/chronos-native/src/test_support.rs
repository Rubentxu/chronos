//! Test-only support for `chronos-native`.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Serialises every test that traces a real child process.
///
/// # Why this exists
///
/// `PtraceTracer::wait_event` has two branches:
///
/// * `follow_children: true` (or no main pid) → `waitpid(-1, __WALL)`, which
///   reaps **any** child of the current process.
/// * `follow_children: false` → `waitpid(pid, WNOHANG)`, scoped to one pid.
///
/// In production that is correct and intentional: `ChronosServer` keeps a
/// single `active_session` (`Arc<Mutex<Option<String>>>`), so exactly one tracer
/// follows exactly one process tree, and `waitpid(-1)` is the right way to see
/// every thread and clone of that tree.
///
/// In the test binary it is not. `cargo test` runs tests in parallel threads
/// **inside one process**, so a `waitpid(-1, __WALL)` from a `capture_runner`
/// test consumes the exit status of the `/bin/true` that a `ptrace_tracer`
/// test is tracing. The other test then never observes its `Exited` event and
/// fails or blocks in `waitpid` forever.
///
/// This is why the symptom is load-sensitive and why the local gate hides it:
/// the gate runs `cargo test --workspace --lib -- --test-threads=1`, which
/// serialises everything. Plain `cargo test` has no such guarantee.
///
/// The fix is to hold this lock for the whole body of any test that traces a
/// child. It is test-only and must never be used to guard production paths:
/// doing so would paper over the real constraint, which is that this crate
/// supports one active trace session per process.
pub(crate) static TRACE_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Acquire [`TRACE_TEST_LOCK`], tolerating a poisoned mutex.
///
/// A panic inside a guarded test would poison the mutex, and every later test
/// would then fail with a confusing poison error instead of running. Clearing
/// the poison keeps the remaining tests meaningful, which is the only reason to
/// be lenient here.
pub(crate) fn lock_trace_test() -> MutexGuard<'static, ()> {
    TRACE_TEST_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}
