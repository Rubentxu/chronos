// Test fixture for DWARF variable extraction tests
// Compile with: gcc -g -o test_dwarf_vars test_dwarf_vars.c

#include <stdio.h>

int global_var = 42;

int simple_function(int param1, int param2) {
    int local_sum = param1 + param2;
    int local_product = param1 * param2;
    int local_diff = param1 - param2;
    char local_char = 'A';
    double local_double = 3.14159;
    int *local_ptr = &local_sum;

    if (local_sum > 10) {
        int conditional_var = local_sum * 2;
        (void)conditional_var;
    }

    for (int i = 0; i < 3; i++) {
        int loop_var = i * local_sum;
        (void)loop_var;
    }

    return local_sum;
}

void no_params_function(void) {
    int standalone = 100;
    int another = 200;
    (void)standalone;
    (void)another;
}

int main(void) {
    int result = simple_function(5, 3);
    printf("Result: %d\n", result);

    no_params_function();

    return 0;
}
