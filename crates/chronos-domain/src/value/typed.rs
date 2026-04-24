//! Variable types and scopes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Scope of a variable.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VariableScope {
    Local,
    Global,
    Closure,
    Static,
    ThreadLocal,
    Parameter,
}

impl std::fmt::Display for VariableScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VariableScope::Local => write!(f, "local"),
            VariableScope::Global => write!(f, "global"),
            VariableScope::Closure => write!(f, "closure"),
            VariableScope::Static => write!(f, "static"),
            VariableScope::ThreadLocal => write!(f, "thread_local"),
            VariableScope::Parameter => write!(f, "parameter"),
        }
    }
}

/// Information about a captured variable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VariableInfo {
    /// Variable name.
    pub name: String,
    /// String representation of the value.
    pub value: String,
    /// Type name (e.g., "i32", "*mut u8").
    pub type_name: String,
    /// Memory address where the variable is stored.
    pub address: u64,
    /// Scope of the variable.
    pub scope: VariableScope,
}

impl VariableInfo {
    /// Create a new variable info.
    pub fn new(
        name: impl Into<String>,
        value: impl Into<String>,
        type_name: impl Into<String>,
        address: u64,
        scope: VariableScope,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            type_name: type_name.into(),
            address,
            scope,
        }
    }

    /// Create a parameter variable.
    pub fn parameter(
        name: impl Into<String>,
        value: impl Into<String>,
        type_name: impl Into<String>,
        address: u64,
    ) -> Self {
        Self::new(name, value, type_name, address, VariableScope::Parameter)
    }

    /// Create a local variable.
    pub fn local(
        name: impl Into<String>,
        value: impl Into<String>,
        type_name: impl Into<String>,
        address: u64,
    ) -> Self {
        Self::new(name, value, type_name, address, VariableScope::Local)
    }
}

/// A typed value with optional member expansion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TypedValue {
    /// String representation of the value.
    pub value: String,
    /// Type name.
    pub type_name: String,
    /// Whether the value is null.
    pub is_null: bool,
    /// For composite types: list of members.
    pub members: Option<Vec<VariableInfo>>,
}

impl TypedValue {
    /// Create a simple scalar typed value.
    pub fn scalar(value: impl Into<String>, type_name: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            type_name: type_name.into(),
            is_null: false,
            members: None,
        }
    }

    /// Create a null value.
    pub fn null(type_name: impl Into<String>) -> Self {
        Self {
            value: "null".to_string(),
            type_name: type_name.into(),
            is_null: true,
            members: None,
        }
    }

    /// Create a composite value with members.
    pub fn composite(
        value: impl Into<String>,
        type_name: impl Into<String>,
        members: Vec<VariableInfo>,
    ) -> Self {
        Self {
            value: value.into(),
            type_name: type_name.into(),
            is_null: false,
            members: Some(members),
        }
    }
}

/// A DWARF location expression result.
///
/// Represents where a variable is located: in a register, in memory,
/// or as an immediate value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DwarfValue {
    /// Variable lives in a named register.
    Register(String),
    /// Variable lives at a memory address.
    Memory { address: u64, size: u32 },
    /// Immediate value (constant).
    Immediate(i64),
}

impl DwarfValue {
    /// Get the memory address if this is a memory location.
    pub fn memory_address(&self) -> Option<u64> {
        match self {
            DwarfValue::Memory { address, .. } => Some(*address),
            _ => None,
        }
    }

    /// Get the register name if this is a register location.
    pub fn register_name(&self) -> Option<&str> {
        match self {
            DwarfValue::Register(name) => Some(name),
            _ => None,
        }
    }

    /// Get the immediate value if this is an immediate.
    pub fn immediate(&self) -> Option<i64> {
        match self {
            DwarfValue::Immediate(val) => Some(*val),
            _ => None,
        }
    }

    /// Format as a string for display.
    pub fn format(&self) -> String {
        match self {
            DwarfValue::Register(name) => format!("register({})", name),
            DwarfValue::Memory { address, size } => format!("memory(0x{:x}, {} bytes)", address, size),
            DwarfValue::Immediate(val) => format!("immediate({})", val),
        }
    }
}

/// A snapshot of register values at a point in time.
///
/// Used by the DWARF location evaluator to resolve variables.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegisterSnapshot {
    /// Program counter (instruction pointer).
    pub pc: u64,
    /// Stack pointer.
    pub sp: u64,
    /// Frame pointer.
    pub fp: u64,
    /// General-purpose registers (name → value).
    pub regs: HashMap<String, u64>,
}

impl RegisterSnapshot {
    /// Create a new register snapshot.
    pub fn new(pc: u64, sp: u64, fp: u64) -> Self {
        Self {
            pc,
            sp,
            fp,
            regs: HashMap::new(),
        }
    }

    /// Add a register value.
    pub fn with_reg(mut self, name: impl Into<String>, value: u64) -> Self {
        self.regs.insert(name.into(), value);
        self
    }

    /// Get a register value by name.
    pub fn get(&self, name: &str) -> Option<u64> {
        self.regs.get(name).copied()
    }

    /// Get the program counter.
    pub fn pc(&self) -> u64 {
        self.pc
    }

    /// Get the stack pointer.
    pub fn sp(&self) -> u64 {
        self.sp
    }

    /// Get the frame pointer.
    pub fn fp(&self) -> u64 {
        self.fp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_scope_display() {
        assert_eq!(VariableScope::Local.to_string(), "local");
        assert_eq!(VariableScope::Parameter.to_string(), "parameter");
    }

    #[test]
    fn test_variable_info_new() {
        let var = VariableInfo::new("x", "42", "i32", 0x7FFE1000, VariableScope::Local);
        assert_eq!(var.name, "x");
        assert_eq!(var.value, "42");
        assert_eq!(var.type_name, "i32");
        assert_eq!(var.address, 0x7FFE1000);
        assert_eq!(var.scope, VariableScope::Local);
    }

    #[test]
    fn test_variable_info_convenience_constructors() {
        let param = VariableInfo::parameter("n", "10", "usize", 0x1000);
        assert_eq!(param.scope, VariableScope::Parameter);

        let local = VariableInfo::local("sum", "55", "i32", 0x2000);
        assert_eq!(local.scope, VariableScope::Local);
    }

    #[test]
    fn test_typed_value_scalar() {
        let val = TypedValue::scalar("42", "i32");
        assert_eq!(val.value, "42");
        assert!(!val.is_null);
        assert!(val.members.is_none());
    }

    #[test]
    fn test_typed_value_null() {
        let val = TypedValue::null("*mut u8");
        assert!(val.is_null);
        assert_eq!(val.value, "null");
    }

    #[test]
    fn test_typed_value_composite() {
        let members = vec![
            VariableInfo::new("x", "1", "i32", 0x1000, VariableScope::Local),
            VariableInfo::new("y", "2", "i32", 0x1004, VariableScope::Local),
        ];
        let val = TypedValue::composite("Point { x: 1, y: 2 }", "Point", members);
        assert_eq!(val.members.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let var = VariableInfo::parameter("count", "5", "usize", 0x5000);
        let json = serde_json::to_string(&var).unwrap();
        let deserialized: VariableInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(var, deserialized);
    }
}
