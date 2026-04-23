// Multi-process C program using fork().
// Parent forks a child, child does work and exits, parent waits.
// This exercises PTRACE_O_TRACEFORK.

#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/wait.h>

int main() {
    pid_t pid = fork();

    if (pid < 0) {
        perror("fork");
        return 1;
    }

    if (pid == 0) {
        // Child process
        volatile long sum = 0;
        for (long i = 0; i < 100000; i++) {
            sum += i;
        }
        printf("Child %d done, sum=%ld\n", getpid(), sum);
        return 42;
    }

    // Parent process
    int status;
    if (waitpid(pid, &status, 0) < 0) {
        perror("waitpid");
        return 1;
    }

    if (WIFEXITED(status)) {
        printf("Child exited with code %d\n", WEXITSTATUS(status));
        // Verify child exited with expected code
        return (WEXITSTATUS(status) == 42) ? 0 : 2;
    }

    printf("Child did not exit normally\n");
    return 3;
}
