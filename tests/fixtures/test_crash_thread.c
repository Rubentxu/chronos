// Thread crashes with SIGSEGV.
// Main thread creates a worker thread that dereferences NULL.
// Verifies that ptrace captures the crash in the child thread
// and that the entire process is cleaned up properly.

#include <stdio.h>
#include <pthread.h>
#include <unistd.h>

void* crasher(void* arg) {
    // Small delay to ensure ptrace sees the thread creation event
    usleep(10000); // 10ms
    int* ptr = NULL;
    *ptr = 42; // SIGSEGV here
    return NULL;
}

int main() {
    pthread_t thread;

    if (pthread_create(&thread, NULL, crasher, NULL) != 0) {
        perror("pthread_create");
        return 1;
    }

    // This won't be reached if the crash happens first
    pthread_join(thread, NULL);
    printf("Should not reach here\n");
    return 0;
}
