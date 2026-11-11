//! Abstract Syntax Tree (AST) definitions for spreadsheet formulas.

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::String, vec::Vec};

use crate::model::Coord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,      // +
    Sub,      // -
    Mul,      // *
    Div,      // /
    Mod,      // %
    Pow,      // ^
    Concat,   // &
    Eq,       // =
    NotEq,    // <>
    Lt,       // <
    LtEq,     // <=
    Gt,       // >
    GtEq,     // >=
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg, // -
    Not, // NOT
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Text(String),
    Boolean(bool),
    CellRef(Coord),
    NamedRef(String),
    RangeRef {
        top_left: Coord,
        bottom_right: Coord,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },
}
