// Intentional data race for race detection testing.
//
// KNOWN_BEHAVIOR:
// - Function calls: main -> incrementer (2 threads)
// - Two threads increment a shared counter WITHOUT synchronization
// - RACE CONDITION: unprotected shared mutable state (counter)
// - Final value of counter will be LESS than expected (typically ~2000000 instead of 4000000)
//   because concurrent increments lose updates
//
// EXPECTED EVENTS:
// - Function entry/exit: main, incrementer
// - Thread creation: 2 threads via std::thread::spawn
// - Thread termination: both threads complete
// - DATA RACE: simultaneous writes to counter variable
// - Race detection tools should flag: shared mutable state without synchronization
//
// USEFUL FOR:
// - Testing race condition detection
// - Verifying that time-travel debugging can identify data races
// - Testing that chronos can detect simultaneous writes to same memory address
// - Validating thread sanitizer integration
//
// NOTE: This program is EXPECTED to behave incorrectly due to the data race.
// The incorrect behavior (lost updates) IS the intended test case.

use std::thread;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn incrementer(id: u64) {
    for i in 0..2_000_000 {
        // Intentional data race: no synchronization
        let current = COUNTER.load(Ordering::Relaxed);
        COUNTER.store(current + 1, Ordering::Relaxed);
    }
    println!("Thread {} finished", id);
}

fn main() {
    println!("Starting data race test...");
    println!("Expected final value (without race): 4000000");
    println!("Actual will be less due to lost updates from race condition");

    let handle1 = thread::spawn(|| incrementer(1));
    let handle2 = thread::spawn(|| incrementer(2));

    handle1.join().expect("Thread 1 panicked");
    handle2.join().expect("Thread 2 panicked");

    let final_value = COUNTER.load(Ordering::Relaxed);
    println!("Final counter value: {}", final_value);
    println!("Lost updates: {}", 4_000_000 - final_value);
}
