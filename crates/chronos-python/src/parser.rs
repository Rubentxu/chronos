use serde::Deserialize;
use std::collections::HashMap;
use chronos_domain::{VariableInfo, VariableScope};
use crate::error::PythonError;

#[derive(Debug, Deserialize)]
pub struct RawPythonEvent {
    pub event: String,
    pub name: String,
    pub file: String,
    pub line: u32,
    #[serde(default)]
    pub is_generator: Option<bool>,
    #[serde(default)]
    pub locals: Option<HashMap<String, String>>,
}

pub fn parse_line(line: &str) -> Result<RawPythonEvent, PythonError> {
    Ok(serde_json::from_str(line)?)
}

pub fn locals_to_variable_info(locals: HashMap<String, String>) -> Vec<VariableInfo> {
    locals.into_iter().map(|(name, value)| {
        VariableInfo::new(name, value, String::new(), 0, VariableScope::Local)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_call_event() {
        let json = r#"{"event": "call", "name": "foo", "file": "test.py", "line": 10}"#;
        let event = parse_line(json).unwrap();
        assert_eq!(event.event, "call");
        assert_eq!(event.name, "foo");
        assert_eq!(event.file, "test.py");
        assert_eq!(event.line, 10);
    }

    #[test]
    fn test_parse_return_event() {
        let json = r#"{"event": "return", "name": "bar", "file": "test.py", "line": 20}"#;
        let event = parse_line(json).unwrap();
        assert_eq!(event.event, "return");
        assert_eq!(event.name, "bar");
    }

    #[test]
    fn test_parse_exception_event() {
        let json = r#"{"event": "exception", "name": "baz", "file": "test.py", "line": 30}"#;
        let event = parse_line(json).unwrap();
        assert_eq!(event.event, "exception");
    }

    #[test]
    fn test_parse_with_locals() {
        let json = r#"{"event": "call", "name": "func", "file": "test.py", "line": 5, "locals": {"x": "42", "y": "hello"}}"#;
        let event = parse_line(json).unwrap();
        assert!(event.locals.is_some());
        let locals = event.locals.unwrap();
        assert_eq!(locals.get("x"), Some(&"42".to_string()));
        assert_eq!(locals.get("y"), Some(&"hello".to_string()));
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = parse_line("not valid json");
        assert!(result.is_err());
    }
}
