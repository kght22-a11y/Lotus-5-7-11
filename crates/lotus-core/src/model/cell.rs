//! Cell data model and value representations.

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::String, vec::Vec};

use super::coord::Coord;

#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Empty,
    Number(f64),
    Text(String),
    Boolean(bool),
    Formula {
        expr: String,
        cached_num: Option<f64>,
        cached_text: Option<String>,
    },
    Basic {
        source: String,
    },
    Blob {
        bytes: Vec<u8>,
    },
    Error(String),
}

impl CellValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            CellValue::Empty => "empty",
            CellValue::Number(_) => "number",
            CellValue::Text(_) => "text",
            CellValue::Boolean(_) => "boolean",
            CellValue::Formula { .. } => "formula",
            CellValue::Basic { .. } => "basic",
            CellValue::Blob { .. } => "blob",
            CellValue::Error(_) => "error",
        }
    }
}

/// A discrete unit in the spreadsheet grid.
///
/// In Lotus 5-7-11:
/// - A cell can hold numbers, text, and formulas.
/// - A cell can be a **named blob** holding arbitrary raw file bytes.
/// - A cell can hold embedded BASIC source code.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub coord: Coord,
    pub id: Option<String>,
    pub name: Option<String>,
    pub value: CellValue,
}

impl Cell {
    pub fn new(coord: Coord, value: CellValue) -> Self {
        Self {
            coord,
            id: None,
            name: None,
            value,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn number(coord: Coord, val: f64) -> Self {
        Self::new(coord, CellValue::Number(val))
    }

    pub fn text(coord: Coord, val: impl Into<String>) -> Self {
        Self::new(coord, CellValue::Text(val.into()))
    }

    pub fn boolean(coord: Coord, val: bool) -> Self {
        Self::new(coord, CellValue::Boolean(val))
    }

    pub fn formula(coord: Coord, expr: impl Into<String>) -> Self {
        Self::new(
            coord,
            CellValue::Formula {
                expr: expr.into(),
                cached_num: None,
                cached_text: None,
            },
        )
    }

    pub fn basic(coord: Coord, name: impl Into<String>, source: impl Into<String>) -> Self {
        Self::new(coord, CellValue::Basic { source: source.into() }).with_name(name)
    }

    pub fn blob(coord: Coord, name: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self::new(coord, CellValue::Blob { bytes }).with_name(name)
    }
}
