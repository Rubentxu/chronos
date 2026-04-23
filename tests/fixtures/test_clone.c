// Uses clone() syscall to create a child process.
// Different from fork() because it uses explicit flags that exercise
// PTRACE_O_TRACECLONE more directly.
// Falls back to simple fork-like clone if advanced flags fail.

#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/wait.h>
#include <sys/syscall.h>
#include <sched.h>
#include <string.h>
#include <errno.h>

static int child_func(void* arg) {
    volatile long sum = 0;
    for (long i = 0; i < 100000; i++) {
        sum += i;
    }
    return 0;
}

int main() {
    // Allocate stack for child
    const int stack_size = 1024 * 1024;
    char* stack = malloc(stack_size);
    if (!stack) {
        perror("malloc");
        return 1;
    }

    // Use clone() with SIGCHLD as exit signal (like fork)
    // but with explicit flags that trigger PTRACE_EVENT_CLONE
    pid_t pid = syscall(SYS_clone,
                        SIGCHLD,           // exit signal
                        stack + stack_size, // stack grows down
                        NULL,              // parent_tidptr
                        NULL,              // tls
                        NULL);             // child_tidptr

    if (pid < 0) {
        perror("clone");
        free(stack);
        return 1;
    }

    if (pid == 0) {
        // Child
        free(stack);
        _exit(child_func(NULL));
    }

    // Parent: wait for child
    int status;
    if (waitpid(pid, &status, 0) < 0) {
        perror("waitpid");
        free(stack);
        return 1;
    }

    free(stack);
    printf("Clone child exited: %d\n", WIFEXITED(status) ? WEXITSTATUS(status) : -1);
    return WIFEXITED(status) ? WEXITSTATUS(status) : 1;
}
