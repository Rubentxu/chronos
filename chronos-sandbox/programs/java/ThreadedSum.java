// Java program with threads for thread lifecycle testing.
//
// STATUS: Placeholder - may have runtime availability issues
// (Requires JDK installed and javac in PATH)
//
// KNOWN_BEHAVIOR:
// - Function calls: main -> Worker.run() (in each thread)
// - Creates 3 threads that each compute a partial sum
// - Main thread waits for all workers to complete via Thread.join()
// - Final sum computed by aggregating partial results
//
// EXPECTED EVENTS:
// - Function entry: main, Worker.run
// - Thread creation: 3 Thread.start() events
// - Thread termination: 3 Thread.exit() events (via Thread.join)
// - No race conditions (proper synchronization via join)
// - Syscalls: clone(), futex(), brk(), exit()
//
// USEFUL FOR:
// - Testing Java thread lifecycle event capture
// - Verifying call stacks include Thread.run() frames
// - Testing multi-threaded process debugging

public class ThreadedSum {
    private static final int NUM_THREADS = 3;
    private static final int ITERATIONS_PER_THREAD = 1_000_000;

    static class Worker implements Runnable {
        private final int threadId;
        private long partialSum;

        Worker(int threadId) {
            this.threadId = threadId;
            this.partialSum = 0;
        }

        public void run() {
            for (int i = 0; i < ITERATIONS_PER_THREAD; i++) {
                partialSum += i % 17;
            }
            System.out.println("Thread " + threadId + " complete. Partial sum: " + partialSum);
        }

        public long getPartialSum() {
            return partialSum;
        }
    }

    public static void main(String[] args) {
        System.out.println("Starting threaded sum test...");

        Thread[] threads = new Thread[NUM_THREADS];
        Worker[] workers = new Worker[NUM_THREADS];

        // Create and start threads
        for (int i = 0; i < NUM_THREADS; i++) {
            workers[i] = new Worker(i);
            threads[i] = new Thread(workers[i]);
            threads[i].start();
        }

        // Wait for all threads to complete
        for (int i = 0; i < NUM_THREADS; i++) {
            try {
                threads[i].join();
            } catch (InterruptedException e) {
                System.err.println("Thread interrupted: " + i);
            }
        }

        // Aggregate results
        long total = 0;
        for (int i = 0; i < NUM_THREADS; i++) {
            total += workers[i].getPartialSum();
        }

        System.out.println("All threads complete. Total sum: " + total);
    }
}
