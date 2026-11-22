//! Virtual Machine with cell confinement and transactional write staging.

#[cfg(not(feature = "std"))]
use alloc::{collections::{BTreeMap, BTreeSet}, format, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::{BTreeMap, BTreeSet};

use super::bytecode::{Opcode, Value};
use super::turtle::Turtle;
use crate::model::{CellValue, ColorRgba, Coord, Sheet};

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
    pub turtle: Turtle,
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
            turtle: Turtle::new(),
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
                Opcode::TurtleForward => {
                    let dist = self.pop_f32("FORWARD")?;
                    self.turtle.forward(dist);
                    self.ip += 1;
                }
                Opcode::TurtleBack => {
                    let dist = self.pop_f32("BACK")?;
                    self.turtle.back(dist);
                    self.ip += 1;
                }
                Opcode::TurtleRight => {
                    let angle = self.pop_f32("RIGHT")?;
                    self.turtle.right(angle);
                    self.ip += 1;
                }
                Opcode::TurtleLeft => {
                    let angle = self.pop_f32("LEFT")?;
                    self.turtle.left(angle);
                    self.ip += 1;
                }
                Opcode::TurtlePenUp => {
                    self.turtle.pen_up();
                    self.ip += 1;
                }
                Opcode::TurtlePenDown => {
                    self.turtle.pen_down();
                    self.ip += 1;
                }
                Opcode::TurtleColor => {
                    let val = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let color = match val {
                        Value::Number(n) => ColorRgba::from_u32_rgb(n as u32),
                        Value::StringLit(s) => match s.to_ascii_lowercase().as_str() {
                            "black" => ColorRgba::BLACK,
                            "white" => ColorRgba::WHITE,
                            "red" => ColorRgba::RED,
                            "green" => ColorRgba::GREEN,
                            "blue" => ColorRgba::BLUE,
                            "yellow" => ColorRgba::rgb(255, 255, 0),
                            "cyan" => ColorRgba::rgb(0, 255, 255),
                            "magenta" => ColorRgba::rgb(255, 0, 255),
                            _ => ColorRgba::WHITE,
                        },
                        _ => return Err(VmError::TypeMismatch("Expected color number or string")),
                    };
                    self.turtle.set_color(color);
                    self.ip += 1;
                }
                Opcode::TurtleWidth => {
                    let w = self.pop_f32("WIDTH")?;
                    self.turtle.set_width(w);
                    self.ip += 1;
                }
                Opcode::TurtleHome => {
                    self.turtle.home();
                    self.ip += 1;
                }
                Opcode::TurtleClear => {
                    self.turtle.clear();
                    self.ip += 1;
                }
                Opcode::TurtleSetPos => {
                    let y = self.pop_f32("SETPOS Y")?;
                    let x = self.pop_f32("SETPOS X")?;
                    self.turtle.set_pos(x, y);
                    self.ip += 1;
                }
                Opcode::TurtleSetHeading => {
                    let deg = self.pop_f32("SETHEADING")?;
                    self.turtle.set_heading(deg);
                    self.ip += 1;
                }
                Opcode::TurtleLabel => {
                    let text = self.stack.pop().ok_or(VmError::StackUnderflow)?.to_string_repr();
                    self.turtle.label(&text);
                    self.ip += 1;
                }
                Opcode::CommitTurtle => {
                    let target_val = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let target = self.resolve_coord(&target_val)?;
                    if !self.writable_cells.contains(&target) {
                        return Err(VmError::CellAccessDenied { coord: target, operation: CellOperation::Write });
                    }
                    if let Some(cell) = self.sheet.get_cell(target) {
                        if matches!(cell.value, CellValue::Formula { .. }) {
                            return Err(VmError::WriteToFormulaCell(target));
                        }
                    }
                    let blob_bytes = self.turtle.buffer.to_bytes();
                    self.staged_writes.insert(target, CellValue::Blob { bytes: blob_bytes });
                    self.ip += 1;
                }
                Opcode::Halt => break,
            }
        }

        Ok(self.staged_writes.len())
    }

    fn pop_f32(&mut self, context: &'static str) -> Result<f32, VmError> {
        let val = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        val.as_f64().map(|n| n as f32).ok_or(VmError::TypeMismatch(context))
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
