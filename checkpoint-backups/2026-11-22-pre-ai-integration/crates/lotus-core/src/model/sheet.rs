//! Worksheet representation and cell collection.

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use super::{cell::Cell, coord::Coord, vector::Rect2D};

#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    pub id: String,
    pub name: String,
    pub cells: BTreeMap<Coord, Cell>,
    /// Column widths in logical vector units (overrides default).
    pub col_widths: BTreeMap<u32, f32>,
    /// Row heights in logical vector units (overrides default).
    pub row_heights: BTreeMap<u32, f32>,
    pub default_col_width: f32,
    pub default_row_height: f32,
}

impl Sheet {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            cells: BTreeMap::new(),
            col_widths: BTreeMap::new(),
            row_heights: BTreeMap::new(),
            default_col_width: 100.0,
            default_row_height: 24.0,
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

    /// Calculate the furthest used column and row for active spreadsheet cells,
    /// explicitly excluding far-out binary/media/vector asset rows so that the
    /// Dynamic Table of Contents (TOC) remains anchored directly adjacent to the
    /// visible computational grid.
    pub fn active_grid_bounds(&self) -> Option<(u32, u32)> {
        let mut max_col = 0;
        let mut max_row = 0;
        let mut found = false;

        for cell in self.cells.values() {
            if cell.is_asset() {
                continue;
            }
            found = true;
            if cell.coord.col > max_col {
                max_col = cell.coord.col;
            }
            if cell.coord.row > max_row {
                max_row = cell.coord.row;
            }
        }

        if found {
            Some((max_col, max_row))
        } else {
            None
        }
    }

    /// Retrieve the logical vector width of a column.
    pub fn col_width(&self, col: u32) -> f32 {
        self.col_widths.get(&col).copied().unwrap_or(self.default_col_width)
    }

    /// Set a custom width for a column in logical vector units.
    pub fn set_col_width(&mut self, col: u32, width: f32) {
        self.col_widths.insert(col, width.max(10.0));
    }

    /// Retrieve the logical vector height of a row.
    pub fn row_height(&self, row: u32) -> f32 {
        self.row_heights.get(&row).copied().unwrap_or(self.default_row_height)
    }

    /// Set a custom height for a row in logical vector units.
    pub fn set_row_height(&mut self, row: u32, height: f32) {
        self.row_heights.insert(row, height.max(8.0));
    }

    /// Compute the logical 2D bounding rectangle of a cell, taking into account
    /// custom column widths, custom row heights, and `col_span` / `row_span`.
    pub fn cell_rect(&self, coord: Coord) -> Rect2D {
        // Calculate X offset by summing previous column widths
        let mut min_x = 0.0f32;
        for c in 0..coord.col {
            min_x += self.col_width(c);
        }

        // Calculate Y offset by summing previous row heights
        let mut min_y = 0.0f32;
        for r in 0..coord.row {
            min_y += self.row_height(r);
        }

        let (col_span, row_span) = self
            .get_cell(coord)
            .map(|cell| (cell.col_span, cell.row_span))
            .unwrap_or((1, 1));

        let mut span_width = 0.0f32;
        for c in coord.col..(coord.col + col_span) {
            span_width += self.col_width(c);
        }

        let mut span_height = 0.0f32;
        for r in coord.row..(coord.row + row_span) {
            span_height += self.row_height(r);
        }

        Rect2D::new(min_x, min_y, min_x + span_width, min_y + span_height)
    }

    /// Determines if an overlapping cell (either a Large Cell with span > 1x1 or
    /// a Curtain overlay) is concealing/obfuscating underlying cells that contain data.
    ///
    /// When true, the user interface renders a thin red line at the bottom of the
    /// anchor cell to inform the user that calculation data is tucked underneath.
    pub fn cell_obfuscates_data(&self, coord: Coord) -> bool {
        let cell = match self.get_cell(coord) {
            Some(c) => c,
            None => return false,
        };

        let has_span = cell.col_span > 1 || cell.row_span > 1;
        let has_overlay = cell.overlay.is_some();

        if !has_span && !has_overlay {
            return false;
        }

        // Inspect all covered coordinates in the span, skipping the anchor cell itself
        for c in coord.col..(coord.col + cell.col_span) {
            for r in coord.row..(coord.row + cell.row_span) {
                if c == coord.col && r == coord.row {
                    continue;
                }
                let covered_coord = Coord::new(c, r);
                if let Some(covered_cell) = self.get_cell(covered_coord) {
                    if !covered_cell.value.is_empty() {
                        return true;
                    }
                }
            }
        }

        false
    }
}
