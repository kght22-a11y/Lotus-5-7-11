//! Virtual Machine with cell confinement and transactional write staging.

#[cfg(not(feature = "std"))]
use alloc::{collections::{BTreeMap, BTreeSet}, format, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::{BTreeMap, BTreeSet};

use super::bytecode::{Opcode, Value};
use crate::model::{CellValue, Coord, Sheet};

#[derive(Debug, PartialEq)]
pub enum VmError {
    StepBudgetExceeded,
    CellAccessDenied { coord: Coord, operation: CellOperation },
    WriteToFormulaCell(Coord),
    StackUnderflow,
    TypeMismatch(&'static str),
    DivisionByZero,
    InvalidCellTarget(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellOperation {
    Read,
    Write,
}

pub struct Vm<'a> {
    sheet: &'a Sheet,
    code: &'a [Opcode],
    ip: usize,
    stack: Vec<Value>,
    vars: BTreeMap<String, Value>,
    steps: usize,
    max_steps: usize,
    pub staged_writes: BTreeMap<Coord, CellValue>,
    readable_cells: BTreeSet<Coord>,
    writable_cells: BTreeSet<Coord>,
}

impl<'a> Vm<'a> {
    pub fn new(sheet: &'a Sheet, code: &'a [Opcode]) -> Self {
        Self {
            sheet,
            code,
            ip: 0,
            stack: Vec::new(),
            vars: BTreeMap::new(),
            steps: 0,
            max_steps: 100_000,
            staged_writes: BTreeMap::new(),
            readable_cells: BTreeSet::new(),
            writable_cells: BTreeSet::new(),
        }
    }

    pub fn with_step_limit(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    /// Grant this procedure access to its own cell and explicitly listed cells.
    pub fn with_cell_access(mut self, owner: Coord, readable: &[Coord], writable: &[Coord]) -> Self {
        self.readable_cells.insert(owner);
        self.writable_cells.insert(owner);
        self.readable_cells.extend(readable.iter().copied());
        self.writable_cells.extend(writable.iter().copied());
        self
    }

    pub fn run(&mut self) -> Result<usize, VmError> {
        while self.ip < self.code.len() {
            self.steps += 1;
            if self.steps > self.max_steps {
                return Err(VmError::StepBudgetExceeded);
            }

            match &self.code[self.ip] {
                Opcode::Push(val) => {
                    self.stack.push(val.clone());
                    self.ip += 1;
                }
                Opcode::Load(name) => {
                    let val = self.vars.get(name).cloned().unwrap_or(Value::Nil);
                    self.stack.push(val);
                    self.ip += 1;
                }
                Opcode::Store(name) => {
                    let val = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    self.vars.insert(name.clone(), val);
                    self.ip += 1;
                }
                Opcode::GetCell => {
                    let target = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let coord = self.resolve_coord(&target)?;
                    if !self.readable_cells.contains(&coord) {
                        return Err(VmError::CellAccessDenied { coord, operation: CellOperation::Read });
                    }
                    let val = self.read_cell_value(coord);
                    self.stack.push(val);
                    self.ip += 1;
                }
                Opcode::SetCell => {
                    let val = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let target = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let coord = self.resolve_coord(&target)?;

                    if !self.writable_cells.contains(&coord) {
                        return Err(VmError::CellAccessDenied { coord, operation: CellOperation::Write });
                    }
                    if matches!(self.sheet.get_cell(coord).map(|cell| &cell.value), Some(CellValue::Formula { .. })) {
                        return Err(VmError::WriteToFormulaCell(coord));
                    }

                    // Convert Value to CellValue and stage it
                    let cell_val = match val {
                        Value::Number(n) => CellValue::Number(n),
                        Value::StringLit(s) => CellValue::Text(s),
                        Value::Boolean(b) => CellValue::Boolean(b),
                        Value::Nil => CellValue::Empty,
                    };
                    self.staged_writes.insert(coord, cell_val);
                    self.ip += 1;
                }
                Opcode::Add => {
                    let r = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let l = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match (l.as_f64(), r.as_f64()) {
                        (Some(a), Some(b)) => self.stack.push(Value::Number(a + b)),
                        _ => self.stack.push(Value::StringLit(format!("{}{}", l.to_string_repr(), r.to_string_repr()))),
                    }
                    self.ip += 1;
                }
                Opcode::Sub => {
                    let r = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let l = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match (l.as_f64(), r.as_f64()) {
                        (Some(a), Some(b)) => self.stack.push(Value::Number(a - b)),
                        _ => return Err(VmError::TypeMismatch("Expected numbers for '-'")),
                    }
                    self.ip += 1;
                }
                Opcode::Mul => {
                    let r = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let l = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match (l.as_f64(), r.as_f64()) {
                        (Some(a), Some(b)) => self.stack.push(Value::Number(a * b)),
                        _ => return Err(VmError::TypeMismatch("Expected numbers for '*'")),
                    }
                    self.ip += 1;
                }
                Opcode::Div => {
                    let r = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let l = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match (l.as_f64(), r.as_f64()) {
                        (Some(_), Some(b)) if b == 0.0 => return Err(VmError::DivisionByZero),
                        (Some(a), Some(b)) => self.stack.push(Value::Number(a / b)),
                        _ => return Err(VmError::TypeMismatch("Expected numbers for '/'")),
                    }
                    self.ip += 1;
                }
                Opcode::Eq => {
                    let r = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let l = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    self.stack.push(Value::Boolean(l == r));
                    self.ip += 1;
                }
                Opcode::NotEq => {
                    let r = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let l = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    self.stack.push(Value::Boolean(l != r));
                    self.ip += 1;
                }
                Opcode::Lt => {
                    let r = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let l = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match (l.as_f64(), r.as_f64()) {
                        (Some(a), Some(b)) => self.stack.push(Value::Boolean(a < b)),
                        _ => self.stack.push(Value::Boolean(l.to_string_repr() < r.to_string_repr())),
                    }
                    self.ip += 1;
                }
                Opcode::LtEq => {
                    let r = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let l = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match (l.as_f64(), r.as_f64()) {
                        (Some(a), Some(b)) => self.stack.push(Value::Boolean(a <= b)),
                        _ => self.stack.push(Value::Boolean(l.to_string_repr() <= r.to_string_repr())),
                    }
                    self.ip += 1;
                }
                Opcode::Gt => {
                    let r = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let l = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match (l.as_f64(), r.as_f64()) {
                        (Some(a), Some(b)) => self.stack.push(Value::Boolean(a > b)),
                        _ => self.stack.push(Value::Boolean(l.to_string_repr() > r.to_string_repr())),
                    }
                    self.ip += 1;
                }
                Opcode::GtEq => {
                    let r = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let l = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match (l.as_f64(), r.as_f64()) {
                        (Some(a), Some(b)) => self.stack.push(Value::Boolean(a >= b)),
                        _ => self.stack.push(Value::Boolean(l.to_string_repr() >= r.to_string_repr())),
                    }
                    self.ip += 1;
                }
                Opcode::Jump(target) => {
                    self.ip = *target;
                }
                Opcode::JumpIfFalse(target) => {
                    let cond = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    if !cond.is_truthy() {
                        self.ip = *target;
                    } else {
                        self.ip += 1;
                    }
                }
                Opcode::Print => {
                    let _ = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    self.ip += 1;
                }
                Opcode::Halt => break,
            }
        }

        Ok(self.staged_writes.len())
    }

    fn resolve_coord(&self, val: &Value) -> Result<Coord, VmError> {
        match val {
            Value::StringLit(s) => {
                if let Some(c) = Coord::parse_a1(s) {
                    return Ok(c);
                }
                // Try resolving named cell
                for cell in self.sheet.cells.values() {
                    if cell.name.as_deref() == Some(s) {
                        return Ok(cell.coord);
                    }
                }
                Err(VmError::InvalidCellTarget(s.clone()))
            }
            _ => Err(VmError::InvalidCellTarget(val.to_string_repr())),
        }
    }

    fn read_cell_value(&self, coord: Coord) -> Value {
        // First check locally staged writes
        if let Some(staged) = self.staged_writes.get(&coord) {
            return match staged {
                CellValue::Number(n) => Value::Number(*n),
                CellValue::Text(t) => Value::StringLit(t.clone()),
                CellValue::Boolean(b) => Value::Boolean(*b),
                _ => Value::Nil,
            };
        }

        // Read from sheet
        match self.sheet.get_cell(coord) {
            Some(cell) => match &cell.value {
                CellValue::Number(n) => Value::Number(*n),
                CellValue::Text(t) => Value::StringLit(t.clone()),
                CellValue::Boolean(b) => Value::Boolean(*b),
                CellValue::Formula { cached_num, cached_text, cached_bool, .. } => {
                    if let Some(n) = cached_num {
                        Value::Number(*n)
                    } else if let Some(value) = cached_bool {
                        Value::Boolean(*value)
                    } else if let Some(t) = cached_text {
                        Value::StringLit(t.clone())
                    } else {
                        Value::Nil
                    }
                }
                _ => Value::Nil,
            },
            None => Value::Nil,
        }
    }
}
