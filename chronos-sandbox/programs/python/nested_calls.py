// Nested function calls with known call depth.
//
// KNOWN_BEHAVIOR:
// - Function calls: main -> level1 -> level2 -> level3 -> level4 -> level5
// - Each level calls the next until level5 returns
// - Call depth of exactly 5 (or 6 counting main)
// - Returns sum of 1..n computed recursively
//
// EXPECTED EVENTS:
// - Function entry/exit events for each level
// - Clear call stack showing 5-6 stack frames
// - Recursive return unwinding
// - Syscalls: brk/sbrk for allocation, exit()
// - No exceptions
//
// USEFUL FOR:
// - Testing call stack reconstruction
// - Verifying deep call chain capture
// - Testing that time-travel can traverse nested calls
// - Validating causality chain (caller -> callee relationships)

def level5(n):
    """Base case: compute sum of 1..n"""
    total = 0
    for i in range(1, n + 1):
        total += i
    return total

def level4(n):
    """Pass through to level5"""
    return level5(n)

def level3(n):
    """Pass through to level4"""
    return level4(n)

def level2(n):
    """Pass through to level3"""
    return level3(n)

def level1(n):
    """Pass through to level2"""
    return level2(n)

def main():
    print("Starting nested call test...")
    result = level1(100)
    print(f"Sum 1..100 = {result}")
    print(f"Call depth: main -> level1 -> level2 -> level3 -> level4 -> level5")
    print("Nested call test complete.")

if __name__ == "__main__":
    main()
