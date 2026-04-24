// Multi-threaded C program using pthreads.
// Creates 3 threads that do work and join, then exits cleanly.
// This exercises PTRACE_O_TRACECLONE and PtraceEvent handling.
//
// KNOWN_BEHAVIOR:
// - Function calls: main, worker (x3)
// - Thread creation: 3 pthread_create events
// - Thread termination: 3 pthread_join events
// - Syscalls: clone(), wait4(), exit()
// - Exit code: 0

#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>

void* worker(void* arg) {
    int id = *(int*)arg;
    volatile long sum = 0;
    for (long i = 0; i < 100000; i++) {
        sum += i * id;
    }
    return (void*)sum;
}

int main() {
    pthread_t threads[3];
    int ids[3] = {1, 2, 3};

    // Create threads
    for (int i = 0; i < 3; i++) {
        if (pthread_create(&threads[i], NULL, worker, &ids[i]) != 0) {
            perror("pthread_create");
            return 1;
        }
    }

    // Join threads
    for (int i = 0; i < 3; i++) {
        void* result;
        if (pthread_join(threads[i], &result) != 0) {
            perror("pthread_join");
            return 1;
        }
    }

    printf("All threads completed\n");
    return 0;
}
