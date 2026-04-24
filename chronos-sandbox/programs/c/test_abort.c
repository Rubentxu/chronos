// C program that calls abort() for SIGABRT.
// Tests signal-based crash capture.
//
// KNOWN_BEHAVIOR:
// - Function calls: main
// - Expected crash: SIGABRT from abort()
// - Syscalls: exit_group() with signal
// - Exit code: signal 6 (SIGABRT)

#include <stdlib.h>
#include <stdio.h>

int main() {
    printf("about to abort\n");
    fflush(stdout);
    abort();
    return 0;
}
