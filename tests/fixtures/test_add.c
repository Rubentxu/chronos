// Simple C program for testing ptrace capture.
// Calls a few functions, does basic arithmetic, exits cleanly.

#include <stdio.h>
#include <stdlib.h>

int add(int a, int b) {
    return a + b;
}

int multiply(int a, int b) {
    return a * b;
}

int compute(int x) {
    int result = add(x, 10);
    result = multiply(result, 2);
    return result;
}

int main(int argc, char *argv[]) {
    int value = 5;
    int result = compute(value);
    printf("Result: %d\n", result);
    return 0;
}
