// CPU-bound hotspot program with nested loops.
//
// KNOWN_BEHAVIOR:
// - Function calls: main -> compute_hotspot -> inner_loop (many calls)
// - Computes a sum via nested loops: outer loop 1000x, inner loop 1000x
// - Total iterations: 1,000,000 (1000 * 1000)
// - The innermost computation is the "hotspot" — hottest function by CPU cycles
//
// EXPECTED EVENTS:
// - Function entry/exit: main, compute_hotspot, inner_loop
// - The inner_loop function will have highest CPU time due to sheer iteration count
// - No syscalls of interest (pure computation)
// - No exceptions (clean exit)
//
// USEFUL FOR:
// - CPU hotspot identification
// - Profiling and performance analysis
// - Verifying that time-travel debugging can identify hot functions
// - Testing saliency scoring in execution summaries

fn inner_loop(iterations: u64) -> u64 {
    let mut sum: u64 = 0;
    for i in 0..iterations {
        sum = sum.wrapping_add(i % 17); // Mix things up a bit
    }
    sum
}

fn compute_hotspot() -> u64 {
    let mut total: u64 = 0;
    for _ in 0..1000 {
        total = total.wrapping_add(inner_loop(1000));
    }
    total
}

fn main() {
    let result = compute_hotspot();
    println!("Hotspot computation complete. Result: {}", result);
}
