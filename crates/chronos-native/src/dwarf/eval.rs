//! DWARF location expression evaluation.
//!
//! Provides `DwarfLocationEvaluator` trait and `BasicLocationEvaluator` implementation
//! for evaluating DWARF location expressions to resolve variable locations.

use chronos_domain::value::{DwarfValue, RegisterSnapshot};

/// Trait for evaluating DWARF location expressions.
pub trait DwarfLocationEvaluator: Send + Sync {
    /// Evaluate a DWARF location expression.
    ///
    /// Given a slice of DWARF location expression bytes and a register snapshot,
    /// returns the resolved location of a variable.
    ///
    /// Returns `Some(DwarfValue)` if evaluation succeeds.
    /// Returns `None` if the expression cannot be evaluated (unsupported op, etc.).
    fn evaluate(&self, expr: &[u8], regs: &RegisterSnapshot) -> Option<DwarfValue>;
}

/// DWARF register number to name mapping for x86_64.
///
/// DWARF uses a platform-specific register numbering scheme.
/// This mapping converts DWARF register numbers to our register names.
const DWARF_REGISTER_MAP: &[(u8, &str)] = &[
    // General purpose registers (DWARF numbers match System V ABI)
    (0, "rax"),   // Return value / syscall return
    (1, "rdx"),   // 3rd arg / syscall return
    (2, "rcx"),   // 4th arg / counter
    (3, "rbx"),   // base pointer
    (4, "rsi"),   // 2nd arg / source
    (5, "rdi"),   // 1st arg / destination
    (6, "rbp"),   // frame pointer
    (7, "rsp"),   // stack pointer
    (8, "r8"),    // 5th arg
    (9, "r9"),    // 6th arg
    (10, "r10"),
    (11, "r11"),
    (12, "r12"),
    (13, "r13"),
    (14, "r14"),
    (15, "r15"),
    // RIP is special - we track it separately as PC
];

/// Basic location evaluator supporting common DWARF location operations.
///
/// Supports:
/// - `DW_OP_reg0` - `DW_OP_reg31`: Direct register
/// - `DW_OP_fbreg`: Frame base register offset
/// - `DW_OP_breg0` - `DW_OP_breg31`: Register-based offset
/// - `DW_OP_plus_uconst`: Unsigned immediate addition
/// - `DW_OP_stack_value`: Value is on the DWARF stack
/// - `DW_OP_addr`: Address constant
///
/// All other operations return `None` (graceful degradation).
pub struct BasicLocationEvaluator {
    // x86_64 DWARF register mapping
    dwarf_reg_map: &'static [(u8, &'static str)],
}

impl BasicLocationEvaluator {
    /// Create a new BasicLocationEvaluator with default x86_64 register mapping.
    pub fn new() -> Self {
        Self {
            dwarf_reg_map: DWARF_REGISTER_MAP,
        }
    }

    /// Get the register name for a DWARF register number.
    fn dwarf_reg_to_name(&self, dwarf_reg: u8) -> Option<&'static str> {
        self.dwarf_reg_map
            .iter()
            .find(|(num, _)| *num == dwarf_reg)
            .map(|(_, name)| *name)
    }

    /// Evaluate a simple register direct operation (DW_OP_reg0-DW_OP_reg31).
    fn eval_reg(&self, dwarf_reg: u8, regs: &RegisterSnapshot) -> Option<DwarfValue> {
        let name = self.dwarf_reg_to_name(dwarf_reg)?;
        // Verify the register has a value
        if regs.get(name).is_some() {
            Some(DwarfValue::Register(name.to_string()))
        } else {
            None
        }
    }

    /// Evaluate DW_OP_fbreg (frame base register offset).
    ///
    /// DW_OP_fbreg N means: address = frame_base + N
    /// On x86_64, the frame base is typically RBP (but can be different).
    fn eval_fbreg(&self, offset: i64, regs: &RegisterSnapshot) -> Option<DwarfValue> {
        // Frame base is usually RBP, but we check FP first
        let base = regs.fp();
        let addr = (base as i64 + offset) as u64;
        Some(DwarfValue::Memory { address: addr, size: 8 })
    }

    /// Evaluate DW_OP_bregN (register-based offset).
    ///
    /// DW_OP_bregN N means: address = register_N + N
    fn eval_breg(&self, dwarf_reg: u8, offset: i64, regs: &RegisterSnapshot) -> Option<DwarfValue> {
        let name = self.dwarf_reg_to_name(dwarf_reg)?;
        let reg_value = regs.get(name)?;
        let addr = (reg_value as i64 + offset) as u64;
        Some(DwarfValue::Memory { address: addr, size: 8 })
    }

    /// Evaluate DW_OP_plus_uconst (unsigned addition).
    ///
    /// Pops a value from the stack, adds N, pushes result.
    /// For this to work, we need a stack state from prior ops.
    fn eval_plus_uconst(stack: &mut Vec<u64>, addend: u64) -> Option<()> {
        let base = stack.pop()?;
        stack.push(base.wrapping_add(addend));
        Some(())
    }

    /// Evaluate a location expression byte stream.
    fn evaluate_inner(&self, expr: &[u8], regs: &RegisterSnapshot) -> Option<DwarfValue> {
        let mut stack: Vec<u64> = Vec::new();
        let mut i = 0;

        while i < expr.len() {
            let op = expr[i];
            i += 1;

            match op {
                // DW_OP_reg0 - DW_OP_reg31: Direct register
                0x50..=0x6f => {
                    let dwarf_reg = op - 0x50;
                    if let Some(val) = self.eval_reg(dwarf_reg, regs) {
                        return Some(val);
                    } else {
                        return None;
                    }
                }

                // DW_OP_fbreg: Frame base register offset + signedLEB128
                0x91 => {
                    let (offset, consumed) = read_sleb128(expr, i);
                    if consumed == 0 { return None; }
                    i += consumed;
                    return self.eval_fbreg(offset, regs);
                }

                // DW_OP_breg0 - DW_OP_breg31: Register + signedLEB128
                0x70..=0x8f => {
                    let dwarf_reg = op - 0x70;
                    let (offset, consumed) = read_sleb128(expr, i);
                    if consumed == 0 { return None; }
                    i += consumed;
                    return self.eval_breg(dwarf_reg, offset, regs);
                }

                // DW_OP_plus_uconst: Unsigned addition
                0x22 => {
                    let (addend, consumed) = read_uleb128(expr, i);
                    if consumed == 0 { return None; }
                    i += consumed;
                    if Self::eval_plus_uconst(&mut stack, addend).is_none() {
                        return None;
                    }
                }

                // DW_OP_stack_value: Value is on the stack (immediate)
                0x9f => {
                    if let Some(&val) = stack.last() {
                        // The value is the immediate, not an address
                        return Some(DwarfValue::Immediate(val as i64));
                    }
                    return None;
                }

                // DW_OP_addr: Address constant (absolute address)
                0x03 => {
                    if expr.len() - i < 8 {
                        return None;
                    }
                    let addr = u64::from_le_bytes([
                        expr[i], expr[i + 1], expr[i + 2], expr[i + 3],
                        expr[i + 4], expr[i + 5], expr[i + 6], expr[i + 7],
                    ]);
                    i += 8;
                    return Some(DwarfValue::Memory { address: addr, size: 8 });
                }

                // DW_OP_deref: Memory dereference (NOT SUPPORTED - would need to read memory)
                0x06 => return None,

                // DW_OP_dup: Duplicate stack top (for completeness)
                0x12 => {
                    if let Some(top) = stack.last() {
                        stack.push(*top);
                    } else {
                        return None;
                    }
                }

                // DW_OP_drop: Remove stack top
                0x13 => {
                    if stack.pop().is_none() {
                        return None;
                    }
                }

                // DW_OP_over: Copy second stack item to top
                0x14 => {
                    if stack.len() >= 2 {
                        let val = stack[stack.len() - 2];
                        stack.push(val);
                    } else {
                        return None;
                    }
                }

                // DW_OP_swap: Swap top two stack items
                0x19 => {
                    if stack.len() >= 2 {
                        let len = stack.len();
                        stack.swap(len - 1, len - 2);
                    } else {
                        return None;
                    }
                }

                // DW_OP_rot: Rotate top three stack items
                0x1a => {
                    if stack.len() >= 3 {
                        let len = stack.len();
                        let top = stack[len - 1];
                        let mid = stack[len - 2];
                        let third = stack[len - 3];
                        stack[len - 1] = third;
                        stack[len - 2] = top;
                        stack[len - 3] = mid;
                    } else {
                        return None;
                    }
                }

                // DW_OP_and, DW_OP_or, DW_OP_xor (binary ops)
                0x1b | 0x1c | 0x1d => {
                    if stack.len() < 2 { return None; }
                    let a = stack.pop()?;
                    let b = stack.pop()?;
                    let result = match op {
                        0x1b => a & b,  // and
                        0x1c => a | b,  // or
                        0x1d => a ^ b,  // xor
                        _ => unreachable!(),
                    };
                    stack.push(result);
                }

                // DW_OP_plus, DW_OP_minus, DW_OP_mul, DW_OP_div, DW_OP_mod, DW_OP_minus
                0x1e..=0x23 => {
                    if stack.len() < 2 { return None; }
                    let a = stack.pop()?;
                    let b = stack.pop()?;
                    let result = match op {
                        0x1e => b.wrapping_add(a),  // plus
                        0x1f => b.wrapping_sub(a),  // minus
                        0x20 => b.wrapping_mul(a),  // mul
                        0x21 => b.wrapping_div(a),  // div
                        0x22 => b.wrapping_rem(a),  // mod
                        0x23 => b.wrapping_shl(a as u32),  // shl
                        _ => unreachable!(),
                    };
                    stack.push(result);
                }

                // DW_OP_ge, DW_OP_gt, DW_OP_le, DW_OP_lt, DW_OP_eq
                0x28..=0x2c => {
                    if stack.len() < 2 { return None; }
                    let a = stack.pop()?;
                    let b = stack.pop()?;
                    let result: u64 = match op {
                        0x28 => if b >= a { 1 } else { 0 },  // ge
                        0x29 => if b > a { 1 } else { 0 },   // gt
                        0x2a => if b <= a { 1 } else { 0 },  // le
                        0x2b => if b < a { 1 } else { 0 },   // lt
                        0x2c => if b == a { 1 } else { 0 },  // eq
                        _ => unreachable!(),
                    };
                    stack.push(result);
                }

                // DW_OP_ne
                0x2d => {
                    if stack.len() < 2 { return None; }
                    let a = stack.pop()?;
                    let b = stack.pop()?;
                    stack.push(if b != a { 1 } else { 0 });
                }

                // Literals 0-31 (DW_OP_lit0 - DW_OP_lit31)
                0x30..=0x4f => {
                    stack.push((op - 0x30) as u64);
                }

                // Unsupported operation
                _ => return None,
            }
        }

        // If we have a stack value and no specific return, the expression
        // resulted in a value on the stack
        if !stack.is_empty() {
            // Return the top of stack as immediate
            return Some(DwarfValue::Immediate(stack.last().unwrap().clone() as i64));
        }

        None
    }
}

impl Default for BasicLocationEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl DwarfLocationEvaluator for BasicLocationEvaluator {
    fn evaluate(&self, expr: &[u8], regs: &RegisterSnapshot) -> Option<DwarfValue> {
        // Empty expression cannot be evaluated
        if expr.is_empty() {
            return None;
        }
        self.evaluate_inner(expr, regs)
    }
}

/// Read an unsigned LEB128 encoded value.
fn read_uleb128(data: &[u8], start: usize) -> (u64, usize) {
    let mut result = 0u64;
    let mut shift = 0;
    let mut i = start;

    while i < data.len() {
        let byte = data[i];
        i += 1;
        result |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return (result, i - start);
        }
        shift += 7;
    }

    (0, 0) // Invalid encoding
}

/// Read a signed LEB128 encoded value.
fn read_sleb128(data: &[u8], start: usize) -> (i64, usize) {
    let mut result = 0i64;
    let mut shift = 0;
    let mut i = start;

    while i < data.len() {
        let byte = data[i];
        i += 1;
        result |= ((byte & 0x7f) as i64) << shift;
        if byte & 0x80 == 0 {
            // Sign extend if necessary
            if shift + 7 < 64 && (byte & 0x40) != 0 {
                result |= -1i64 << (shift + 7);
            }
            return (result, i - start);
        }
        shift += 7;
    }

    (0, 0) // Invalid encoding
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_regs(pc: u64, sp: u64, fp: u64) -> RegisterSnapshot {
        let mut regs = HashMap::new();
        regs.insert("rax".to_string(), 0);
        regs.insert("rbx".to_string(), 0x100);
        regs.insert("rcx".to_string(), 0x200);
        regs.insert("rdx".to_string(), 0x300);
        regs.insert("rsi".to_string(), 0x400);
        regs.insert("rdi".to_string(), 0x500);
        regs.insert("rbp".to_string(), fp);
        regs.insert("rsp".to_string(), sp);
        regs.insert("r8".to_string(), 0x800);
        regs.insert("r9".to_string(), 0x900);

        RegisterSnapshot { pc, sp, fp, regs }
    }

    #[test]
    fn test_dwarf_reg_to_name() {
        let evaluator = BasicLocationEvaluator::new();
        assert_eq!(evaluator.dwarf_reg_to_name(0), Some("rax"));
        assert_eq!(evaluator.dwarf_reg_to_name(5), Some("rdi"));
        assert_eq!(evaluator.dwarf_reg_to_name(6), Some("rbp"));
        assert_eq!(evaluator.dwarf_reg_to_name(7), Some("rsp"));
        assert_eq!(evaluator.dwarf_reg_to_name(8), Some("r8"));
        assert_eq!(evaluator.dwarf_reg_to_name(15), Some("r15"));
        assert_eq!(evaluator.dwarf_reg_to_name(16), None); // Invalid
    }

    #[test]
    fn test_eval_reg0() {
        let evaluator = BasicLocationEvaluator::new();
        let mut regs = make_regs(0x400000, 0x7fff0000, 0x7fff8000);
        regs.regs.insert("rax".to_string(), 0x7fff1234);

        // DW_OP_reg0 should return rax
        let result = evaluator.evaluate(&[0x50], &regs);
        assert!(matches!(result, Some(DwarfValue::Register(name)) if name == "rax"));
    }

    #[test]
    fn test_eval_reg5() {
        let evaluator = BasicLocationEvaluator::new();
        let regs = make_regs(0x400000, 0x7fff0000, 0x7fff8000);

        // DW_OP_reg5 should return rdi
        let result = evaluator.evaluate(&[0x55], &regs);
        assert!(matches!(result, Some(DwarfValue::Register(name)) if name == "rdi"));
    }

    #[test]
    fn test_eval_fbreg_minus_8() {
        let evaluator = BasicLocationEvaluator::new();
        let regs = make_regs(0x400000, 0x7fff0000, 0x7fff8000);

        // DW_OP_fbreg -8 = frame_base - 8 = 0x7fff8000 - 8 = 0x7fff7ff8
        // 0x91 is DW_OP_fbreg, then we need SLEB128 encoding of -8
        // -8 in SLEB128 is 0x78 (continuation bit set, value bits = 8)
        let expr = vec![0x91, 0x78];
        let result = evaluator.evaluate(&expr, &regs);
        assert!(matches!(result, Some(DwarfValue::Memory { address, size: 8 }) if address == 0x7fff7ff8));
    }

    #[test]
    fn test_eval_breg0_plus_16() {
        let evaluator = BasicLocationEvaluator::new();
        let mut regs = make_regs(0x400000, 0x7fff0000, 0x7fff8000);
        regs.regs.insert("rax".to_string(), 0x1000);

        // DW_OP_breg0 +16 = rax + 16 = 0x1000 + 16 = 0x1010
        // 0x70 is DW_OP_breg0, then SLEB128 of +16
        // 16 in SLEB128 is 0x10 (no continuation)
        let expr = vec![0x70, 0x10];
        let result = evaluator.evaluate(&expr, &regs);
        assert!(matches!(result, Some(DwarfValue::Memory { address, size: 8 }) if address == 0x1010));
    }

    #[test]
    fn test_eval_plus_uconst() {
        let evaluator = BasicLocationEvaluator::new();
        let regs = make_regs(0x400000, 0x7fff0000, 0x7fff8000);

        // First push a value (e.g., DW_OP_lit5 = 0x35), then DW_OP_plus_uconst 3
        // 0x35 = DW_OP_lit5, 0x22 = DW_OP_plus_uconst, 0x03 = 3
        let expr = vec![0x35, 0x22, 0x03];
        let result = evaluator.evaluate(&expr, &regs);
        // 5 + 3 = 8, so result should be immediate(8)
        assert!(matches!(result, Some(DwarfValue::Immediate(8))));
    }

    #[test]
    fn test_eval_stack_value() {
        let evaluator = BasicLocationEvaluator::new();
        let regs = make_regs(0x400000, 0x7fff0000, 0x7fff8000);

        // Push a value and use DW_OP_stack_value
        // 0x35 = DW_OP_lit5, 0x9f = DW_OP_stack_value
        let expr = vec![0x35, 0x9f];
        let result = evaluator.evaluate(&expr, &regs);
        assert!(matches!(result, Some(DwarfValue::Immediate(5))));
    }

    #[test]
    fn test_eval_addr() {
        let evaluator = BasicLocationEvaluator::new();
        let regs = make_regs(0x400000, 0x7fff0000, 0x7fff8000);

        // DW_OP_addr 0x1000 (little-endian: 0x00 0x10 0x00 0x00 0x00 0x00 0x00 0x00)
        let expr = vec![0x03, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let result = evaluator.evaluate(&expr, &regs);
        assert!(matches!(result, Some(DwarfValue::Memory { address: 0x1000, size: 8 })));
    }

    #[test]
    fn test_unsupported_op_returns_none() {
        let evaluator = BasicLocationEvaluator::new();
        let regs = make_regs(0x400000, 0x7fff0000, 0x7fff8000);

        // DW_OP_deref (0x06) is not supported
        let expr = vec![0x06];
        let result = evaluator.evaluate(&expr, &regs);
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_expression_returns_none() {
        let evaluator = BasicLocationEvaluator::new();
        let regs = make_regs(0x400000, 0x7fff0000, 0x7fff8000);

        let result = evaluator.evaluate(&[], &regs);
        assert!(result.is_none());
    }

    #[test]
    fn test_literal_push() {
        let evaluator = BasicLocationEvaluator::new();
        let regs = make_regs(0x400000, 0x7fff0000, 0x7fff8000);

        // DW_OP_lit0 = 0x30
        let expr = vec![0x30];
        let result = evaluator.evaluate(&expr, &regs);
        assert!(matches!(result, Some(DwarfValue::Immediate(0))));

        // DW_OP_lit15 = 0x3f
        let expr15 = vec![0x3f];
        let result15 = evaluator.evaluate(&expr15, &regs);
        assert!(matches!(result15, Some(DwarfValue::Immediate(15))));
    }

    #[test]
    fn test_read_uleb128() {
        // 127 = single byte 0x7f
        let (val, consumed) = read_uleb128(&[0x7f], 0);
        assert_eq!(val, 127);
        assert_eq!(consumed, 1);

        // 128 = two bytes 0x80 0x01
        let (val, consumed) = read_uleb128(&[0x80, 0x01], 0);
        assert_eq!(val, 128);
        assert_eq!(consumed, 2);

        // 300 = two bytes 0xac 0x02
        let (val, consumed) = read_uleb128(&[0xac, 0x02], 0);
        assert_eq!(val, 300);
        assert_eq!(consumed, 2);
    }

    #[test]
    fn test_read_sleb128() {
        // -1 = single byte 0x7f (continuation bit set, value bits = 1, sign extend)
        let (val, consumed) = read_sleb128(&[0x7f], 0);
        assert_eq!(val, -1);
        assert_eq!(consumed, 1);

        // -8 = single byte 0x78 (continuation bit set, value bits = 8)
        let (val, consumed) = read_sleb128(&[0x78], 0);
        assert_eq!(val, -8);
        assert_eq!(consumed, 1);

        // 8 = single byte 0x08
        let (val, consumed) = read_sleb128(&[0x08], 0);
        assert_eq!(val, 8);
        assert_eq!(consumed, 1);
    }
}
