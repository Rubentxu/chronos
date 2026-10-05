// C program that receives a fatal signal, handles it, and exits normally.
//
// This is the fixture that makes a crash verdict falsifiable. The other crash
// fixtures (test_abort, test_divide_by_zero) die, so they confirm only that
// the detector fires. This one is the opposite case: the program receives
// SIGSEGV and SIGABRT through raise(), handles both, and returns 0.
//
// A verdict built on "a fatal signal was delivered" reports a crash here. The
// tracee did not crash: it logged, kept running, and returned 0. Only a
// verdict that rests on the tracee's termination can tell those apart.
//
// EXPECTED:
// - The signal is delivered twice, so two SignalDelivered events exist.
// - The program survives both and returns exit code 0.
// - Therefore: no crash. `find_crash` must answer crash_found = false.
//
// KNOWN_BEHAVIOR:
// - Function calls: main, handle_fatal
// - Expected crash: none, by construction
// - Exit code: 0

#define _GNU_SOURCE
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

static volatile sig_atomic_t handled = 0;

static void handle_fatal(int sig) {
    // A real handler would unwind or inspect state. Writing to stderr is async
    // signal-safe and enough to prove the handler ran.
    handled++;
    (void)!write(STDERR_FILENO, "handled\n", 8);
}

int main() {
    struct sigaction sa;
    sa.sa_handler = handle_fatal;
    sigemptyset(&sa.sa_mask);
    sa.sa_flags = SA_RESTART;

    if (sigaction(SIGSEGV, &sa, NULL) != 0 || sigaction(SIGABRT, &sa, NULL) != 0) {
        perror("sigaction");
        return 1;
    }

    printf("raising SIGSEGV\n");
    fflush(stdout);
    raise(SIGSEGV);

    printf("still alive after SIGSEGV, raising SIGABRT\n");
    fflush(stdout);
    raise(SIGABRT);

    printf("handled %d signals, exiting cleanly\n", (int)handled);
    fflush(stdout);
    return 0;
}
