// Real-function-capture fixture for m2 native INT3 breakpoint injection.
//
// Compiled at test time with `-no-pie -O0 -fno-inline` (and DWARF) so the
// SymbolResolver finds `main`, `add` and `fact` as size-bearing text symbols
// whose entry addresses the tracer can breakpoint and single-step through.
// `fact` recurses so a single capture must observe several distinct
// invocations of the same function address.
//
// `sink` is volatile so the optimizer cannot drop the calls at -O0.

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
