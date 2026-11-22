//! Dynamic Table of Contents (TOC) positioned at (maxX + 1, maxY + 1).

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use super::coord::Coord;

#[derive(Debug, Clone, PartialEq)]
pub struct TocEntry {
    pub sheet_id: String,
    pub coord: Coord,
    pub name: String,
    pub type_name: String,
    pub byte_size: usize,
}

/// The Dynamic Table of Contents.
///
/// In Lotus 5-7-11, the TOC is not at a static location.
/// It dynamically sits one cell beyond the furthest used column and row:
/// `pos = (maxX + 1, maxY + 1)`.
/// E.g., if used cells are within columns A..C (0..2) and rows 1..26 (0..25),
/// the TOC resides at D27 (col 3, row 26).
#[derive(Debug, Clone, PartialEq)]
pub struct TableOfContents {
    pub pos: Coord,
    pub extent_col: u32,
    pub extent_row: u32,
    pub entries: Vec<TocEntry>,
}
