#!/usr/bin/env python3
"""Capability probe: can we ptrace-attach to an arbitrary pid right now?

Exits 0 if PTRACE_ATTACH succeeded, 1 otherwise (reason on stdout).
This is the provenance for the `ptrace` capability in s0run results.
"""
import ctypes
import ctypes.util
import sys

PTRACE_ATTACH = 16
libc_path = ctypes.util.find_library("c") or "libc.so.6"
try:
    libc = ctypes.CDLL(libc_path, use_errno=True)
except OSError as exc:
    print(f"libc-load-failed: {exc}")
    sys.exit(1)

pid = int(sys.argv[1])
ctypes.set_errno(0)
rc = libc.ptrace(PTRACE_ATTACH, pid, None, None)
if rc == 0:
    # Detach immediately so we do not disturb the tracee.
    libc.ptrace(17, pid, None, None)  # PTRACE_DETACH
    sys.exit(0)
errno = ctypes.get_errno()
print(f"ptrace-attach-denied errno={errno}")
sys.exit(1)
