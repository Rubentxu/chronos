// C program that crashes with SIGSEGV for testing crash capture.
//
// KNOWN_BEHAVIOR:
// - Function calls: cause_crash(), main
// - Expected crash: SIGSEGV in cause_crash() at NULL dereference
// - Syscalls: exit_group() with signal
// - Exit code: signal 11 (SIGSEGV)

#include <stdlib.h>

int cause_crash() {
    int *ptr = NULL;
    *ptr = 42;  // SIGSEGV here
    return 0;
}

int main() {
    cause_crash();
    return 0;
}
