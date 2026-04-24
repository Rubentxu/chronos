// Long-running busy loop for testing ptrace capture.
// Keeps the CPU busy for ~3 seconds, generating function entry/exit events.
//
// KNOWN_BEHAVIOR:
// - Function calls: main, compute_loop, do_work
// - Duration: ~3 seconds
// - Syscalls: getpid (every 10 iterations), gettimeofday (clock), exit
// - Exit code: 0

#include <stdio.h>
#include <stdlib.h>
#include <sys/time.h>
#include <unistd.h>
#include <time.h>

void do_work(int iter) {
    volatile long sum = 0;
    // Each iteration burns ~10ns
    for (long i = 0; i < 1000000; i++) {
        sum += i * iter;
    }
    // Make a syscall every 10 iterations to generate trace events
    if (iter % 10 == 0) {
        getpid();
    }
}

void compute_loop(int seconds) {
    clock_t start = clock();
    int iter = 0;
    while (((clock() - start) / CLOCKS_PER_SEC) < seconds) {
        do_work(iter);
        iter++;
    }
    printf("Completed %d iterations in %d seconds\n", iter, seconds);
}

int main() {
    compute_loop(3);
    return 0;
}