/// Bootstrap Python code for tracing.
///
/// The target script is NOT part of this code. It is read back from
/// `sys.argv[1]` at runtime, so a target path can never be interpreted as
/// Python source no matter what characters it contains.
///
/// The target runs in a fresh `__main__` namespace. That matters for more than
/// style: the tracer returns the global `_chronos_trace` on every event, so a
/// target that assigns that name would break tracing if it shared the
/// bootstrap's namespace.
///
/// `runpy.run_path` would give the same isolation, but its first call performs
/// a large lazy import of the import machinery *after* the tracer is already
/// installed. That put roughly 750 bootstrap frames in front of the target's
/// own events, where the previous implementation produced about ten. Reading
/// the file and exec'ing it into a private dict keeps the stream identical.
pub fn bootstrap_code() -> &'static str {
    r#"
import sys, json, os

_capture_locals = os.environ.get("CHRONOS_CAPTURE_LOCALS", "1") == "1"

def _chronos_trace(frame, event, arg):
    if event not in ("call", "return", "exception"):
        return _chronos_trace
    info = {
        "event": event,
        "name": frame.f_code.co_qualname if hasattr(frame.f_code, "co_qualname") else frame.f_code.co_name,
        "file": frame.f_code.co_filename,
        "line": frame.f_lineno,
        "is_generator": bool(frame.f_code.co_flags & 0x20),
    }
    if event == "call" and _capture_locals:
        locs = {}
        for k, v in frame.f_locals.items():
            try:
                s = repr(v)
                locs[k] = s[:256] if len(s) > 256 else s
            except Exception:
                locs[k] = "<error>"
        info["locals"] = locs
    print(json.dumps(info), flush=True)
    return _chronos_trace

sys.settrace(_chronos_trace)

# Execute the target script passed as command-line argument
_target = sys.argv[1] if len(sys.argv) > 1 else None
if _target:
    with open(_target, "r") as _f:
        _script_code = _f.read()
    _ns = {"__name__": "__main__", "__file__": _target}
    exec(compile(_script_code, _target, "exec"), _ns)
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_code_includes_settrace() {
        let code = bootstrap_code();
        assert!(
            code.contains("sys.settrace"),
            "Bootstrap code should call sys.settrace"
        );
        assert!(
            code.contains("_chronos_trace"),
            "Bootstrap code should define _chronos_trace"
        );
        assert!(
            code.contains("json.dumps"),
            "Bootstrap code should output JSON"
        );
    }

    /// The target is data, never code.
    ///
    /// This module used to expose a second constructor that took a target and
    /// spliced it into the Python source, escaping only backslashes. A target
    /// containing `"` or a newline rewrote the generated program. It was
    /// reachable from `PythonSubprocess::spawn`, which passed
    /// `CaptureConfig::target` straight into it.
    ///
    /// The invariant is now structural rather than escaped: there is no
    /// parameter to inject through, and the only channel from a target into the
    /// interpreter is `sys.argv[1]`.
    #[test]
    fn test_bootstrap_code_never_embeds_a_target() {
        let code = bootstrap_code();
        assert!(
            code.contains("sys.argv[1]"),
            "the target must be read from argv, not from the source"
        );
        assert!(
            !code.contains("r\"{"),
            "the program must not contain a raw string placeholder for the target"
        );
        assert!(
            code.contains("exec(compile(_script_code, _target, \"exec\"), _ns)"),
            "the target must be exec'd in a private namespace, not this one"
        );
        assert!(
            code.contains("\"__name__\": \"__main__\""),
            "the private namespace must present itself as __main__"
        );
    }
}
