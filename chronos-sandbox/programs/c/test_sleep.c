// C program that stays alive without producing trace events.
// Used to exercise the "tracee alive but not producing" path.
//
// KNOWN_BEHAVIOR:
// - Function calls: main, sleep
// - Expected: one nanosleep syscall, then blocked for the whole run
// - No crash: the only way this process ends is an external signal
//
// Why this fixture exists: every other fixture either terminates quickly or
// floods the tracer with syscalls (`test_infinite_loop` calls getpid() in a
// tight loop, so with syscall tracing on it produces an event per iteration).
// Neither shape can hold a tracer in the idle state, which is the state the
// stall report in `PtraceTracer::wait_event` exists for, and the state whose
// polling cost has to be measured rather than assumed.
//
// The duration is read from argv so one binary covers both a short capture
// and a window long enough to cross the stall threshold, instead of shipping
// two near-identical fixtures that could drift apart.

#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int main(int argc, char **argv) {
    // Default long enough to sit well past the 30 s stall threshold, so a
    // capture that never produces anything reaches the reporting path.
    unsigned int seconds = 120;
    if (argc > 1) {
        seconds = (unsigned int)strtoul(argv[1], NULL, 10);
    }

    printf("sleeping %u s\n", seconds);
    fflush(stdout);

    // One syscall, then the tracee is blocked in the kernel and the tracer
    // sees no ptrace status at all. That is the state being measured.
    sleep(seconds);
    return 0;
}
