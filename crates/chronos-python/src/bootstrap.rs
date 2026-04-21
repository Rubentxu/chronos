/// Bootstrap Python code for tracing.
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
import runpy
_target = sys.argv[1] if len(sys.argv) > 1 else None
if _target:
    runpy.run_path(_target, run_name='__main__')
"#
}

/// Bootstrap Python code for tracing a specific target.
/// The target path is embedded in the bootstrap code itself.
pub fn bootstrap_code_for_target(target: &str) -> String {
    // Escape backslashes for Windows paths and regular paths
    let escaped_target = target.replace('\\', "\\\\");
    format!(r#"
import sys, json, os

_capture_locals = os.environ.get("CHRONOS_CAPTURE_LOCALS", "1") == "1"

def _chronos_trace(frame, event, arg):
    if event not in ("call", "return", "exception"):
        return _chronos_trace
    info = {{
        "event": event,
        "name": frame.f_code.co_qualname if hasattr(frame.f_code, "co_qualname") else frame.f_code.co_name,
        "file": frame.f_code.co_filename,
        "line": frame.f_lineno,
        "is_generator": bool(frame.f_code.co_flags & 0x20),
    }}
    if event == "call" and _capture_locals:
        locs = {{}}
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

# Execute the target script
with open(r"{escaped_target}", "r") as f:
    script_code = f.read()
exec(compile(script_code, r"{escaped_target}", "exec"))
"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_code_includes_settrace() {
        let code = bootstrap_code();
        assert!(code.contains("sys.settrace"), "Bootstrap code should call sys.settrace");
        assert!(code.contains("_chronos_trace"), "Bootstrap code should define _chronos_trace");
        assert!(code.contains("json.dumps"), "Bootstrap code should output JSON");
    }
}
