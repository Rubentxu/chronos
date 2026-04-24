// Chained goroutines for goroutine lifecycle testing.
//
// KNOWN_BEHAVIOR:
// - Creates a chain of goroutines: main -> spawner -> worker -> nested
// - Each goroutine spawns the next one with a channel communication
// - Chain depth of 3 goroutines (not counting main)
// - Each goroutine does some work and signals completion via channel
//
// EXPECTED EVENTS:
// - Function entry: main, spawner, worker, nested
// - Goroutine creation: go spawner, go worker, go nested (3 events)
// - Channel send/receive operations (4 events: spawner->worker, worker->nested, completion signals)
// - Goroutine termination: all three goroutines complete
// - Syscalls: clone(), futex(), exit()
//
// USEFUL FOR:
// - Testing goroutine creation/termination event capture
// - Verifying channel operation interception
// - Testing call graph reconstruction across goroutines
// - Validating causality chains across async boundaries

package main

import (
	"fmt"
	"sync"
)

func nested(wg *sync.WaitGroup, ch chan int) {
	defer wg.Done()
	result := 0
	for i := 0; i < 1000; i++ {
		result += i % 17
	}
	ch <- result
	fmt.Println("Nested goroutine complete")
}

func worker(wg *sync.WaitGroup, ch chan int) {
	defer wg.Done()
	wgNested := &sync.WaitGroup{}
	wgNested.Add(1)
	chNested := make(chan int, 1)
	go nested(wgNested, chNested)
	result := <-chNested
	ch <- result
	fmt.Println("Worker goroutine complete")
}

func spawner(wg *sync.WaitGroup, ch chan int) {
	defer wg.Done()
	wgWorker := &sync.WaitGroup{}
	wgWorker.Add(1)
	chWorker := make(chan int, 1)
	go worker(wgWorker, chWorker)
	result := <-chWorker
	ch <- result
	fmt.Println("Spawner goroutine complete")
}

func main() {
	fmt.Println("Starting goroutine chain test...")

	wg := &sync.WaitGroup{}
	wg.Add(1)
	ch := make(chan int, 1)
	go spawner(wg, ch)

	result := <-ch
	wg.Wait()
	fmt.Printf("Goroutine chain complete. Result: %d\n", result)
	fmt.Println("Call chain: main -> spawner -> worker -> nested")
}
