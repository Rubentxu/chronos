// PIE-flagged real-function-capture fixture for m2 native INT3 breakpoint
// injection. Compiled at test time with `-pie -fPIE -O0 -fno-inline` (and
// DWARF) so the SymbolResolver finds `main`, `add` and `fact` as
// size-bearing text symbols whose runtime addresses the tracer must relocate
// via the ASLR load-bias path.
//
// `fact` recurses so a single capture must observe several distinct
// invocations of the same function address.
//
// `sink` is volatile so the optimizer cannot drop the calls at -O0.
//
// Intentionally a near-copy of `test_function_frames.c` so the only
// difference between the two fixtures is the PIE flag; everything else
// (symbol layout, recursion depth, function count) is identical. The
// chronos-native integration test asserts that `compute_load_bias` returns
// a non-zero value when run against this binary.

__attribute__((noinline)) int add(int a, int b) { return a + b; }

__attribute__((noinline)) int fact(int n) {
    if (n <= 1) {
        return 1;
    }
    return n * fact(n - 1);
}

static volatile int sink;

int main(void) {
    sink = add(2, 3);   // one entry
    sink = fact(4);     // four recursive entries (n = 4, 3, 2, 1)
    return 0;
}
