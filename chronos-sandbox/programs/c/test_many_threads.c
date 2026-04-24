// Stress test: creates many threads simultaneously.
// Exercises ptrace tracer's ability to handle rapid clone events.
// All threads do work and join cleanly.
//
// KNOWN_BEHAVIOR:
// - Function calls: main, worker (x10)
// - Thread creation: 10 pthread_create events
// - Thread termination: 10 pthread_join events
// - Syscalls: clone(), wait4(), exit()
// - Exit code: 0

#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>

#define NUM_THREADS 10

void* worker(void* arg) {
    int id = *(int*)arg;
    volatile long sum = 0;
    for (long i = 0; i < 50000; i++) {
        sum += i * id;
    }
    return (void*)sum;
}

int main() {
    pthread_t threads[NUM_THREADS];
    int ids[NUM_THREADS];

    // Create all threads at once
    for (int i = 0; i < NUM_THREADS; i++) {
        ids[i] = i + 1;
        if (pthread_create(&threads[i], NULL, worker, &ids[i]) != 0) {
            perror("pthread_create");
            return 1;
        }
    }

    // Join all threads
    long total = 0;
    for (int i = 0; i < NUM_THREADS; i++) {
        void* result;
        if (pthread_join(threads[i], &result) != 0) {
            perror("pthread_join");
            return 1;
        }
        total += (long)result;
    }

    printf("All %d threads completed, total=%ld\n", NUM_THREADS, total);
    return 0;
}
