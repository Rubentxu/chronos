// Chained exceptions for exception detection testing.
//
// KNOWN_BEHAVIOR:
// - Function calls: main -> func_a -> func_b -> func_c
// - func_c raises ValueError("error_c")
// - func_b catches and re-raises as RuntimeError("error_b") wrapping error_c
// - func_a catches and re-raises as RuntimeError("error_a") wrapping error_b
// - main catches the final exception and prints the full chain
//
// EXPECTED EVENTS:
// - Function entry: main, func_a, func_b, func_c
// - Exception raised: ValueError in func_c
// - Exception caught + re-raised: RuntimeError in func_b, RuntimeError in func_a
// - Exception caught: final RuntimeError in main
// - Exception chain: error_a -> error_b -> error_c (causality chain)
// - Syscalls: brk/sbrk for allocation, exit()
//
// USEFUL FOR:
// - Testing exception/caught-exception event capture
// - Verifying causality chain reconstruction (original cause preserved)
// - Testing that time-travel can identify the root cause of an exception
// - Validating exception stack trace reconstruction

class CustomException(Exception):
    """Base exception for this test"""
    pass

def func_c():
    """Raise the original error"""
    raise ValueError("error_c")

def func_b():
    """Catch func_c's error and wrap it"""
    try:
        func_c()
    except ValueError as e:
        raise RuntimeError("error_b") from e

def func_a():
    """Catch func_b's error and wrap it again"""
    try:
        func_b()
    except RuntimeError as e:
        raise RuntimeError("error_a") from e

def main():
    print("Starting exception chain test...")
    try:
        func_a()
    except RuntimeError as e:
        print(f"Caught exception: {e}")
        print(f"Cause chain:")
        cause = e.__cause__
        while cause is not None:
            print(f"  -> {type(cause).__name__}: {cause}")
            cause = cause.__cause__
        print("Exception chain test complete.")
    except Exception as e:
        print(f"Unexpected exception type: {type(e).__name__}: {e}")

if __name__ == "__main__":
    main()
