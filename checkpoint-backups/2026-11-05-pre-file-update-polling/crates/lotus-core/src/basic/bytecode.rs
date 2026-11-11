//! Instruction set and value representation for the Lotus Modern BASIC bytecode VM.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    StringLit(String),
    Boolean(bool),
    Nil,
}

impl Value {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
            Value::StringLit(s) => s.parse::<f64>().ok(),
            Value::Nil => None,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Boolean(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::StringLit(s) => !s.is_empty(),
            Value::Nil => false,
        }
    }

    pub fn to_string_repr(&self) -> String {
        match self {
            Value::Number(n) => format!("{}", n),
            Value::StringLit(s) => s.clone(),
            Value::Boolean(b) => (if *b { "TRUE" } else { "FALSE" }).into(),
            Value::Nil => "NIL".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    Push(Value),
    Load(String),
    Store(String),
    GetCell,
    SetCell,
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Jump(usize),
    JumpIfFalse(usize),
    Print,
    Halt,
}
