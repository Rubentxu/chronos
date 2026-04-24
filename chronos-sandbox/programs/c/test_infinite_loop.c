// C program with an infinite loop.
// This should be killed by probe_stop to test stop on timeout.
//
// KNOWN_BEHAVIOR:
// - Infinite loop calling getpid() syscall
// - Should be terminated by external signal

#include <unistd.h>

int main() {
    while(1) {
        getpid();
    }
    return 0;
}
