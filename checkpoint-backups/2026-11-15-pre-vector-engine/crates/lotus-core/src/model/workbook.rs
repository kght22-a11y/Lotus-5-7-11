//! Workbook container and spatial extent calculator.

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, format, string::String, string::ToString, vec, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use super::{
    cell::{Cell, CellValue},
    coord::Coord,
    sheet::Sheet,
    toc::{TableOfContents, TocEntry},
};
use crate::{
    calc::ast::Expr,
    doc::{
        BrokenDocCellRef, BrokenDocRef, BrokenSectionRef, DanglingFormulaRef,
        Document, DocumentRefStatus, OrphanReport,
    },
    history::HistoryEngine,
};

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

    /// Validate an outgoing document reference (e.g. "briefing" or "briefing#inputs"):
    /// - Checks whether the document ID exists.
    /// - If a section fragment is present, verifies that the document contains a matching heading.
    pub fn validate_document_ref(&self, target: &str) -> DocumentRefStatus {
        let (doc_id, fragment) = match target.split_once('#') {
            Some((id, frag)) => (id.trim(), Some(frag.trim())),
            None => (target.trim(), None),
        };
        let Some(doc) = self.get_document(doc_id) else {
            return DocumentRefStatus::UnresolvedDocument;
        };
        if let Some(frag) = fragment {
            if frag.is_empty() {
                DocumentRefStatus::Valid
            } else if let Some(heading) = doc.find_section(frag) {
                DocumentRefStatus::ValidWithSection(heading.title)
            } else {
                DocumentRefStatus::UnresolvedSection(frag.to_string())
            }
        } else {
            DocumentRefStatus::Valid
        }
    }

    /// Audit the workbook for orphaned entities and broken references across:
    /// 1. Outgoing cell document references pointing to missing documents or missing section anchors.
    /// 2. Document-embedded live cell templates (`{{cell:...}}`) pointing to missing sheets or missing cells.
    /// 3. Embedded documents with zero incoming cell references.
    /// 4. Formula cells with dangling dependencies on non-existent or empty cells.
    pub fn check_orphans(&self) -> OrphanReport {
        let mut report = OrphanReport::default();

        // 1. Broken cell -> document references
        for sheet in &self.sheets {
            for cell in sheet.cells.values() {
                for target in &cell.document_refs {
                    match self.validate_document_ref(target) {
                        DocumentRefStatus::UnresolvedDocument => {
                            let doc_id = target.split_once('#').map_or(target.as_str(), |(id, _)| id);
                            report.broken_document_refs.push(BrokenDocRef {
                                sheet_id: sheet.id.clone(),
                                coord: cell.coord,
                                cell_name: cell.name.clone(),
                                target: target.clone(),
                                reason: format!("Document '{}' not found", doc_id),
                            });
                        }
                        DocumentRefStatus::UnresolvedSection(frag) => {
                            let doc_id = target.split_once('#').map_or(target.as_str(), |(id, _)| id);
                            report.broken_section_refs.push(BrokenSectionRef {
                                sheet_id: sheet.id.clone(),
                                coord: cell.coord,
                                cell_name: cell.name.clone(),
                                document_id: doc_id.to_string(),
                                section_anchor: frag,
                                target: target.clone(),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }

        // 2. Broken document -> cell references
        let default_sheet_id = self.primary_sheet().map(|s| s.id.as_str()).unwrap_or("");
        for doc in &self.documents {
            for target in doc.referenced_cells() {
                let (sheet_id, cell_ref) = target.split_once('!')
                    .unwrap_or((default_sheet_id, target.as_str()));
                let Some(sheet) = self.get_sheet(sheet_id) else {
                    report.broken_document_cell_refs.push(BrokenDocCellRef {
                        document_id: doc.id.clone(),
                        target: target.clone(),
                        reason: format!("Worksheet '{}' not found", sheet_id),
                    });
                    continue;
                };
                let found = if let Some(coord) = Coord::parse_a1(cell_ref) {
                    sheet.get_cell(coord).is_some()
                } else {
                    sheet.cells.values().any(|c| c.name.as_deref() == Some(cell_ref))
                };
                if !found {
                    report.broken_document_cell_refs.push(BrokenDocCellRef {
                        document_id: doc.id.clone(),
                        target: target.clone(),
                        reason: format!("Cell '{}' not found on sheet '{}'", cell_ref, sheet_id),
                    });
                }
            }
        }

        // 3. Unreferenced documents (documents not referenced by any cell's document_refs)
        for doc in &self.documents {
            let is_referenced = self.sheets.iter().any(|sheet| {
                sheet.cells.values().any(|cell| {
                    cell.document_refs.iter().any(|target| {
                        document_ref_id(target) == doc.id
                    })
                })
            });
            if !is_referenced {
                report.unreferenced_documents.push(doc.id.clone());
            }
        }

        // 4. Dangling formula dependencies
        for sheet in &self.sheets {
            for cell in sheet.cells.values() {
                if let CellValue::Formula { expr, .. } = &cell.value {
                    if let Ok(ast) = crate::calc::Parser::parse_str(expr) {
                        let mut raw_targets = Vec::new();
                        collect_raw_formula_targets(&ast, &mut raw_targets);
                        for raw_target in raw_targets {
                            match raw_target {
                                RawFormulaTarget::Coord(coord) => {
                                    if sheet.get_cell(coord).is_none() {
                                        report.dangling_formula_refs.push(DanglingFormulaRef {
                                            sheet_id: sheet.id.clone(),
                                            coord: cell.coord,
                                            formula_name: cell.name.clone(),
                                            target: format!("{}", coord),
                                        });
                                    }
                                }
                                RawFormulaTarget::Name(name) => {
                                    if !sheet.cells.values().any(|c| c.name.as_deref() == Some(&name)) {
                                        report.dangling_formula_refs.push(DanglingFormulaRef {
                                            sheet_id: sheet.id.clone(),
                                            coord: cell.coord,
                                            formula_name: cell.name.clone(),
                                            target: name,
                                        });
                                    }
                                }
                                RawFormulaTarget::Range(tl, br) => {
                                    let mut any_cell_found = false;
                                    for col in tl.col..=br.col {
                                        for row in tl.row..=br.row {
                                            if sheet.get_cell(Coord::new(col, row)).is_some() {
                                                any_cell_found = true;
                                                break;
                                            }
                                        }
                                        if any_cell_found {
                                            break;
                                        }
                                    }
                                    if !any_cell_found {
                                        report.dangling_formula_refs.push(DanglingFormulaRef {
                                            sheet_id: sheet.id.clone(),
                                            coord: cell.coord,
                                            formula_name: cell.name.clone(),
                                            target: format!("{}:{}", tl, br),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        report
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

enum RawFormulaTarget {
    Coord(Coord),
    Name(String),
    Range(Coord, Coord),
}

fn collect_raw_formula_targets(expr: &Expr, targets: &mut Vec<RawFormulaTarget>) {
    match expr {
        Expr::CellRef(c) => targets.push(RawFormulaTarget::Coord(*c)),
        Expr::NamedRef(name) => targets.push(RawFormulaTarget::Name(name.clone())),
        Expr::RangeRef { top_left, bottom_right } => {
            targets.push(RawFormulaTarget::Range(*top_left, *bottom_right));
        }
        Expr::Unary { expr, .. } => collect_raw_formula_targets(expr, targets),
        Expr::Binary { left, right, .. } => {
            collect_raw_formula_targets(left, targets);
            collect_raw_formula_targets(right, targets);
        }
        Expr::FunctionCall { args, .. } => {
            for arg in args {
                collect_raw_formula_targets(arg, targets);
            }
        }
        _ => {}
    }
}

fn document_ref_id(target: &str) -> &str {
    target.split_once('#').map_or(target, |(document_id, _)| document_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_ref_validation() {
        let doc = Document::new(
            "spec",
            "Specifications",
            "# Overview\nIntro.\n\n## Section One\nDetails.",
        );
        let wb = Workbook::new("w1", "Test").with_document(doc);

        assert_eq!(wb.validate_document_ref("spec"), DocumentRefStatus::Valid);
        assert_eq!(
            wb.validate_document_ref("spec#section-one"),
            DocumentRefStatus::ValidWithSection("Section One".into())
        );
        assert_eq!(
            wb.validate_document_ref("spec#Section One"),
            DocumentRefStatus::ValidWithSection("Section One".into())
        );
        assert_eq!(
            wb.validate_document_ref("spec#missing"),
            DocumentRefStatus::UnresolvedSection("missing".into())
        );
        assert_eq!(
            wb.validate_document_ref("unknown_doc#overview"),
            DocumentRefStatus::UnresolvedDocument
        );
    }

    #[test]
    fn test_orphan_check_identifies_broken_refs() {
        let mut sheet = Sheet::new("main", "Main");
        // A1 links to a non-existent document
        sheet.set_cell(
            Cell::number(Coord::new(0, 0), 10.0)
                .with_name("rev")
                .with_document_refs(vec!["missing_doc".into(), "briefing#missing_section".into()]),
        );
        // A2 has formula referencing nonexistent cell Z99
        sheet.set_cell(Cell::formula(Coord::new(0, 1), "=Z99 + 5"));

        // Document referencing nonexistent cell A50
        let doc = Document::new("briefing", "Briefing", "# Title\n{{cell:A50}}");
        // Unreferenced document
        let unref_doc = Document::new("notes", "Notes", "# Notes\nSome notes");

        let wb = Workbook::new("w1", "Test")
            .with_sheet(sheet)
            .with_document(doc)
            .with_document(unref_doc);

        let report = wb.check_orphans();
        assert_eq!(report.broken_document_refs.len(), 1);
        assert_eq!(report.broken_document_refs[0].target, "missing_doc");

        assert_eq!(report.broken_section_refs.len(), 1);
        assert_eq!(report.broken_section_refs[0].section_anchor, "missing_section");

        assert_eq!(report.broken_document_cell_refs.len(), 1);
        assert_eq!(report.broken_document_cell_refs[0].target, "A50");

        assert_eq!(report.unreferenced_documents, vec!["notes"]);
        assert_eq!(report.dangling_formula_refs.len(), 1);
        assert_eq!(report.dangling_formula_refs[0].target, "Z99");
    }
}
