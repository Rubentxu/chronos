// C program that causes SIGFPE (floating point exception).
// Triggers division by zero to test crash capture.
//
// KNOWN_BEHAVIOR:
// - Function calls: main
// - Expected crash: SIGFPE from division by zero
// - Syscalls: exit_group() with signal
// - Exit code: signal 8 (SIGFPE)

#include <stdio.h>

int main() {
    volatile int x = 0;
    volatile int y = 10 / x;  // triggers SIGFPE
    printf("result: %d\n", y);
    return 0;
}
