//! Worksheet representation and cell collection.

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use super::{cell::Cell, coord::Coord};

#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    pub id: String,
    pub name: String,
    pub cells: BTreeMap<Coord, Cell>,
}

impl Sheet {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            cells: BTreeMap::new(),
        }
    }

    /// Insert or update a cell in the sheet.
    pub fn set_cell(&mut self, cell: Cell) {
        self.cells.insert(cell.coord, cell);
    }

    /// Retrieve a reference to a cell at a specific coordinate.
    pub fn get_cell(&self, coord: Coord) -> Option<&Cell> {
        self.cells.get(&coord)
    }

    /// Retrieve a mutable reference to a cell at a specific coordinate.
    pub fn get_cell_mut(&mut self, coord: Coord) -> Option<&mut Cell> {
        self.cells.get_mut(&coord)
    }

    /// Calculate the furthest used column and row in this sheet.
    /// Returns `None` if the sheet has no cells.
    pub fn bounds(&self) -> Option<(u32, u32)> {
        if self.cells.is_empty() {
            return None;
        }

        let mut max_col = 0;
        let mut max_row = 0;
        for coord in self.cells.keys() {
            if coord.col > max_col {
                max_col = coord.col;
            }
            if coord.row > max_row {
                max_row = coord.row;
            }
        }

        Some((max_col, max_row))
    }
}
