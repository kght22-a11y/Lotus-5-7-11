//! Workbook container and spatial extent calculator.

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use super::{
    cell::{Cell, CellValue},
    coord::Coord,
    sheet::Sheet,
    toc::{TableOfContents, TocEntry},
};
use crate::{doc::Document, history::HistoryEngine};

#[derive(Debug, Clone, PartialEq)]
pub struct Workbook {
    pub id: String,
    pub title: String,
    pub calc_mode: String,
    pub sheets: Vec<Sheet>,
    pub documents: Vec<Document>,
    /// Historical cell overlays, keyed by worksheet ID. The base sheets are Z=0.
    pub history: BTreeMap<String, HistoryEngine>,
}

impl Workbook {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            calc_mode: "auto".into(),
            sheets: Vec::new(),
            documents: Vec::new(),
            history: BTreeMap::new(),
        }
    }

    pub fn with_document(mut self, doc: Document) -> Self {
        self.documents.push(doc);
        self
    }

    pub fn get_document(&self, id: &str) -> Option<&Document> {
        self.documents.iter().find(|d| d.id == id)
    }

    /// Documents explicitly linked from a cell, resolving each target's document ID.
    pub fn documents_linked_from_cell(&self, sheet_id: &str, coord: Coord) -> Vec<&Document> {
        self.documents_linked_from_cell_at_z(sheet_id, coord, 0)
    }

    pub fn documents_linked_from_cell_at_z(&self, sheet_id: &str, coord: Coord, z: u32) -> Vec<&Document> {
        let Some(sheet) = self.sheet_at_z(sheet_id, z) else { return Vec::new() };
        let Some(cell) = sheet.get_cell(coord) else {
            return Vec::new();
        };
        self.documents.iter().filter(|document| {
            cell.document_refs.iter().any(|target| document_ref_id(target) == document.id)
        }).collect()
    }

    /// Documents whose explicit live-cell templates refer to this cell by A1 coordinate or name.
    pub fn documents_referencing_cell(&self, sheet_id: &str, coord: Coord) -> Vec<&Document> {
        self.documents_referencing_cell_at_z(sheet_id, coord, 0)
    }

    pub fn documents_referencing_cell_at_z(&self, sheet_id: &str, coord: Coord, z: u32) -> Vec<&Document> {
        let Some(sheet) = self.sheet_at_z(sheet_id, z) else { return Vec::new() };
        let Some(cell) = sheet.get_cell(coord) else {
            return Vec::new();
        };
        let default_sheet_id = self.primary_sheet().map(|sheet| sheet.id.as_str()).unwrap_or("");
        self.documents.iter()
            .filter(|document| document.references_cell(cell, sheet_id, default_sheet_id))
            .collect()
    }

    pub fn with_sheet(mut self, sheet: Sheet) -> Self {
        self.sheets.push(sheet);
        self
    }

    pub fn primary_sheet(&self) -> Option<&Sheet> {
        self.sheets.first()
    }

    pub fn primary_sheet_mut(&mut self) -> Option<&mut Sheet> {
        self.sheets.first_mut()
    }

    pub fn get_sheet(&self, id: &str) -> Option<&Sheet> {
        self.sheets.iter().find(|s| s.id == id)
    }

    pub fn get_sheet_mut(&mut self, id: &str) -> Option<&mut Sheet> {
        self.sheets.iter_mut().find(|s| s.id == id)
    }

    pub fn history_for_sheet(&self, id: &str) -> Option<&HistoryEngine> {
        self.history.get(id)
    }

    /// Compose a worksheet at Z=0 (the base) or at a stored historical layer.
    pub fn sheet_at_z(&self, id: &str, z: u32) -> Option<Sheet> {
        let base = self.get_sheet(id)?;
        match self.history.get(id) {
            Some(history) if z <= history.max_z => Some(history.composite_at_z(base, z)),
            None if z == 0 => Some(base.clone()),
            _ => None,
        }
    }

    /// Record changes as a new historical layer without modifying the base sheet.
    pub fn record_history_layer(
        &mut self,
        sheet_id: &str,
        changes: Vec<(Coord, Option<Cell>)>,
    ) -> Result<u32, &'static str> {
        if self.get_sheet(sheet_id).is_none() {
            return Err("Sheet not found");
        }
        let history = self.history.entry(sheet_id.into()).or_insert_with(HistoryEngine::new);
        history.record_layer(changes)
    }

    /// Dynamically computes the Table of Contents (TOC) anchored at (maxX + 1, maxY + 1).
    pub fn compute_toc(&self) -> TableOfContents {
        let mut max_col = 0;
        let mut max_row = 0;
        let mut has_cells = false;
        let mut entries = Vec::new();

        for sheet in &self.sheets {
            if let Some((scol, srow)) = sheet.bounds() {
                has_cells = true;
                if scol > max_col {
                    max_col = scol;
                }
                if srow > max_row {
                    max_row = srow;
                }
            }

            for cell in sheet.cells.values() {
                if let Some(ref name) = cell.name {
                    let (type_name, byte_size) = match &cell.value {
                        CellValue::Blob { bytes } => ("blob", bytes.len()),
                        CellValue::Basic { source } => ("basic", source.len()),
                        CellValue::Formula { expr, .. } => ("formula", expr.len()),
                        CellValue::Text(t) => ("text", t.len()),
                        CellValue::Number(_) => ("number", 8),
                        CellValue::Boolean(_) => ("boolean", 1),
                        CellValue::Empty => ("empty", 0),
                        CellValue::Error(e) => ("error", e.len()),
                    };

                    entries.push(TocEntry {
                        sheet_id: sheet.id.clone(),
                        coord: cell.coord,
                        name: name.clone(),
                        type_name: String::from(type_name),
                        byte_size,
                    });
                }
            }
        }

        let (toc_col, toc_row) = if has_cells {
            (max_col + 1, max_row + 1)
        } else {
            (0, 0)
        };

        TableOfContents {
            pos: Coord::new(toc_col, toc_row),
            extent_col: max_col,
            extent_row: max_row,
            entries,
        }
    }

    /// Eject a named cell as an exact byte slice.
    ///
    /// In Lotus 5-7-11:
    /// - "A cell can be a named blob... Its payload is the file's byte sequence.
    ///    Ejecting a cell exports its payload as a standalone file, preserving
    ///    byte order, embedded zero bytes, and exact length."
    pub fn eject_cell(&self, name: &str) -> Option<&[u8]> {
        for doc in &self.documents {
            if doc.id == name {
                return Some(doc.body.as_bytes());
            }
        }

        for sheet in &self.sheets {
            for cell in sheet.cells.values() {
                if cell.name.as_deref() == Some(name) {
                    return match &cell.value {
                        CellValue::Blob { bytes } => Some(bytes.as_slice()),
                        CellValue::Basic { source } => Some(source.as_bytes()),
                        CellValue::Text(t) => Some(t.as_bytes()),
                        _ => None,
                    };
                }
            }
        }
        None
    }

    /// Import a raw byte sequence into a named cell at the given coordinate.
    pub fn import_blob(
        &mut self,
        sheet_idx: usize,
        coord: Coord,
        name: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Result<(), &'static str> {
        let sheet = self.sheets.get_mut(sheet_idx).ok_or("Sheet not found")?;
        sheet.set_cell(Cell::blob(coord, name, bytes));
        Ok(())
    }
}

fn document_ref_id(target: &str) -> &str {
    target.split_once('#').map_or(target, |(document_id, _)| document_id)
}
