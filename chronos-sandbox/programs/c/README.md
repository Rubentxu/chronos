# C Program Fixtures

This directory contains C programs used for testing the Chronos MCP sandbox capture capabilities.

## Programs

### test_add.c
**Purpose**: Simple arithmetic and function call chain

**What it does**:
- Defines `add()`, `multiply()`, and `compute()` functions
- `main()` calls `compute(5)` which does: `add(5, 10) * 2 = 30`
- Prints "Result: 30" and exits cleanly

**Expected Events**:
- Function calls: `main` → `compute` → `add` → `multiply`
- Syscalls: `exit()`

**Exit**: Clean (exit code 0)

---

### test_segfault.c
**Purpose**: Tests crash detection with SIGSEGV

**What it does**:
- Calls `cause_crash()` which dereferences a NULL pointer
- Triggers SIGSEGV

**Expected Events**:
- Function calls: `main` → `cause_crash`
- Crash: SIGSEGV at NULL dereference
- Syscalls: `exit_group()` with signal

**Exit**: Signal 11 (SIGSEGV)

---

### test_threads.c
**Purpose**: Multi-threaded program with 3 threads

**What it does**:
- Creates 3 worker threads that compute a sum
- Each worker does 100,000 iterations of accumulation
- Main thread joins all workers and prints completion message

**Expected Events**:
- Function calls: `main` → `worker` (3x)
- Thread creation: 3 `pthread_create` events
- Thread termination: 3 `pthread_join` events
- Syscalls: `clone()`, `wait4()`, `exit()`

**Exit**: Clean (exit code 0)

---

### test_many_threads.c
**Purpose**: Stress test with 10 threads

**What it does**:
- Creates 10 worker threads simultaneously
- Each worker does 50,000 iterations
- Main thread joins all and prints total

**Expected Events**:
- Function calls: `main` → `worker` (10x)
- Thread creation: 10 `pthread_create` events
- Thread termination: 10 `pthread_join` events
- Syscalls: `clone()`, `wait4()`, `exit()`

**Exit**: Clean (exit code 0)

---

### test_fork.c
**Purpose**: Multi-process program using fork()

**What it does**:
- Parent forks a child process
- Child computes a sum (100,000 iterations) and exits with code 42
- Parent waits for child with `waitpid()` and verifies exit code

**Expected Events**:
- Function calls: `main`
- Process creation: `fork()` creates child
- Child exit: `exit(42)`
- Parent wait: `waitpid()` returns with status
- Syscalls: `clone()`, `wait4()`, `exit()`

**Exit**: Clean (exit code 0, child exited with 42)

---

### test_crash_thread.c
**Purpose**: Thread-level crash detection

**What it does**:
- Creates a worker thread that crashes with SIGSEGV
- Main thread waits to join (though crash happens first)
- Thread dereferences NULL after a 10ms delay

**Expected Events**:
- Function calls: `main` → `crasher`
- Thread creation: 1 `pthread_create` event
- Crash: SIGSEGV in `crasher` thread at NULL dereference
- Syscalls: `clone()`, `wait4()`, `exit_group()` with signal

**Exit**: Signal 11 (SIGSEGV)

---

### test_clone.c
**Purpose**: Uses clone() syscall directly

**What it does**:
- Uses `syscall(SYS_clone, ...)` to create a child process
- Child executes `child_func()` which computes a sum and exits
- Parent waits for child with `waitpid()`
- Uses explicit clone flags that trigger PTRACE_EVENT_CLONE

**Expected Events**:
- Function calls: `main` → `child_func`
- Process creation: `clone()` with explicit flags
- Child exit: via `child_func` return
- Parent wait: `waitpid()` returns with status
- Syscalls: `clone()`, `wait4()`, `exit()`

**Exit**: Clean (exit code 0)

---

## Compilation

All programs are compiled via `build.rs` using the `cc` crate:
- Debug symbols are included (`-g` flag)
- Output: `target/debug/libtest_fixtures.a` (static library)

To compile manually:
```bash
cd programs/c
gcc -g -o test_add test_add.c
gcc -g -o test_segfault test_segfault.c
# etc.
```

## Usage in Tests

These programs are used by integration tests to verify:
- Function call tracing
- Syscall capture
- Crash detection and reporting
- Thread creation/termination tracking
- Multi-process (fork/clone) event capture
