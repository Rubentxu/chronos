// C program that crashes with SIGSEGV for testing crash capture.
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
