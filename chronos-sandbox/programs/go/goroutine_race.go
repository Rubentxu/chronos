// Intentional race condition for race detection testing.
//
// KNOWN_BEHAVIOR:
// - Function calls: main -> incrementer (multiple goroutines)
// - Multiple goroutines increment a shared counter WITHOUT synchronization
// - RACE CONDITION: unprotected shared mutable state (counter variable)
// - Final value will be LESS than expected due to lost updates from data races
//
// EXPECTED EVENTS:
// - Function entry: main, incrementer
// - Goroutine creation: 3 incrementer goroutines
// - DATA RACE: simultaneous unsynchronized writes to shared counter
// - Race detection tools should flag: unsynchronized shared state access
// - Syscalls: clone(), futex(), exit()
//
// USEFUL FOR:
// - Testing race condition detection in Go programs
// - Verifying that time-travel can identify data races across goroutines
// - Testing that chronos can detect simultaneous writes to same memory address
// - Validating goroutine sanitizer integration
//
// NOTE: This program is EXPECTED to behave incorrectly due to the data race.
// The incorrect behavior (lost updates) IS the intended test case.

package main

import (
	"fmt"
	"sync"
)

// Shared counter - intentionally NOT protected by mutex or atomic
var counter int = 0

func incrementer(id int, iterations int) {
	for i := 0; i < iterations; i++ {
		// Intentional race: read, increment, write without synchronization
		temp := counter
		counter = temp + 1
	}
	fmt.Printf("Incrementer %d complete\n", id)
}

func main() {
	fmt.Println("Starting goroutine race test...")
	fmt.Println("Expected final value (without race): 3000000")
	fmt.Println("Actual will be less due to lost updates from race condition")

	var wg sync.WaitGroup
	iterations := 1_000_000

	wg.Add(3)
	go incrementer(1, iterations)
	go incrementer(2, iterations)
	go incrementer(3, iterations)

	wg.Wait()

	fmt.Printf("Final counter value: %d\n", counter)
	fmt.Printf("Lost updates: %d\n", 3*iterations-counter)
}
