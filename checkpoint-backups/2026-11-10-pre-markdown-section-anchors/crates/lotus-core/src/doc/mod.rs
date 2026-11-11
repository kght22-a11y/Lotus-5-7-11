//! Embedded Documents, Cross-References, and Live Templates.
//!
//! As specified in Lotus 5-7-11 Section 6:
//! - Documents (Markdown notes, specifications, reports) are first-class workbook objects.
//! - Cell-to-document references allow cells to link to supporting context.
//! - Document-to-cell references embed live spreadsheet values (e.g. `{{cell:revenue}}` or `{{cell:A1}}`)
//!   or immutable point-in-time snapshots (`{{snapshot:A1}}`).

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, string::ToString, vec::Vec};

use crate::model::{Cell, Coord, Sheet, Workbook};

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub content_type: String, // e.g. "text/markdown"
    pub body: String,
}

impl Document {
    pub fn new(id: impl Into<String>, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            content_type: "text/markdown".into(),
            body: body.into(),
        }
    }

    /// Renders the document body by resolving live cell references:
    /// - `{{cell:A1}}` or `{{cell:name}}` resolves against the current live sheet state.
    pub fn render_live(&self, sheet: &Sheet, _workbook: Option<&Workbook>) -> String {
        let mut out = String::new();
        let chars: Vec<char> = self.body.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if i + 1 < chars.len() && chars[i] == '{' && chars[i + 1] == '{' {
                let start = i + 2;
                let mut end = start;
                while end + 1 < chars.len() && !(chars[end] == '}' && chars[end + 1] == '}') {
                    end += 1;
                }

                if end + 1 < chars.len() {
                    let tag: String = chars[start..end].iter().collect();
                    let resolved = resolve_tag(tag.trim(), sheet, _workbook);
                    out.push_str(&resolved);
                    i = end + 2;
                    continue;
                }
            }
            out.push(chars[i]);
            i += 1;
        }

        out
    }

    /// Return explicit `{{cell:target}}` references in document order.
    pub fn referenced_cells(&self) -> Vec<String> {
        let mut references = Vec::new();
        let mut remaining = self.body.as_str();
        while let Some(start) = remaining.find("{{") {
            remaining = &remaining[start + 2..];
            let Some(end) = remaining.find("}}") else { break };
            let tag = remaining[..end].trim();
            if let Some(target) = tag.strip_prefix("cell:") {
                let target = target.trim();
                if !target.is_empty() {
                    references.push(target.to_string());
                }
            }
            remaining = &remaining[end + 2..];
        }
        references
    }

    /// Whether this document explicitly references the supplied cell by A1 coordinate or name.
    pub fn references_cell(&self, cell: &Cell, sheet_id: &str, default_sheet_id: &str) -> bool {
        self.referenced_cells().iter().any(|target| {
            let (target_sheet_id, target_cell) = target.split_once('!')
                .unwrap_or((default_sheet_id, target.as_str()));
            target_sheet_id == sheet_id
                && (Coord::parse_a1(target_cell).is_some_and(|coord| coord == cell.coord)
                    || cell.name.as_deref() == Some(target_cell))
        })
    }
}

fn resolve_tag(tag: &str, sheet: &Sheet, workbook: Option<&Workbook>) -> String {
    if let Some(target) = tag.strip_prefix("cell:") {
        let target = target.trim();
        let (target_sheet, target) = if let Some((sheet_id, cell_target)) = target.split_once('!') {
            let Some(target_sheet) = workbook.and_then(|workbook| workbook.get_sheet(sheet_id.trim())) else {
                return format!("<unknown-sheet:{}>", sheet_id.trim());
            };
            (target_sheet, cell_target.trim())
        } else {
            (sheet, target)
        };
        // Try coordinate first
        let coord = if let Some(c) = Coord::parse_a1(target) {
            Some(c)
        } else {
            // Try cell name
            target_sheet.cells.values().find(|c| c.name.as_deref() == Some(target)).map(|c| c.coord)
        };

        if let Some(c) = coord {
            if let Some(cell) = target_sheet.get_cell(c) {
                match &cell.value {
                    crate::model::CellValue::Number(n) => format!("{:.2}", n),
                    crate::model::CellValue::Text(t) => t.clone(),
                    crate::model::CellValue::Boolean(b) => (if *b { "TRUE" } else { "FALSE" }).into(),
                    crate::model::CellValue::Formula { cached_num, cached_text, cached_bool, expr } => {
                        if let Some(n) = cached_num {
                            format!("{:.2}", n)
                        } else if let Some(value) = cached_bool {
                            (if *value { "TRUE" } else { "FALSE" }).into()
                        } else if let Some(ref t) = cached_text {
                            t.clone()
                        } else {
                            format!("={}", expr)
                        }
                    }
                    _ => "<unsupported>".into(),
                }
            } else {
                "<empty>".into()
            }
        } else {
            format!("<unknown:{}>", target)
        }
    } else {
        format!("{{{{{}}}}}", tag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Cell, Coord};

    #[test]
    fn test_embedded_document_live_resolution() {
        let mut sheet = Sheet::new("main", "Main");
        sheet.set_cell(Cell::number(Coord::new(0, 0), 12500.5).with_name("revenue"));
        sheet.set_cell(Cell::number(Coord::new(0, 1), 0.25).with_name("tax_rate"));
        sheet.set_cell(Cell::number(Coord::new(0, 2), 3125.125).with_name("tax_amount"));

        let doc_body = r#"# Executive Summary
The model currently projects total revenue of ${{cell:revenue}} (cell A1).
With a declared tax rate of {{cell:A2}}, the estimated tax liability is ${{cell:tax_amount}}.
"#;

        let doc = Document::new("exec-summary", "Executive Summary", doc_body);
        let rendered = doc.render_live(&sheet, None);

        assert!(rendered.contains("total revenue of $12500.50 (cell A1)"));
        assert!(rendered.contains("tax rate of 0.25"));
        assert!(rendered.contains("estimated tax liability is $3125.12"));
    }

    #[test]
    fn document_cell_references_are_exposed_as_explicit_graph_edges() {
        let mut sheet = Sheet::new("main", "Main");
        let cell = Cell::number(Coord::new(0, 0), 42.0)
            .with_name("answer")
            .with_document_refs(vec!["analysis#results".into()]);
        sheet.set_cell(cell);
        let document = Document::new("analysis", "Analysis", "{{cell:answer}} / {{cell:A1}} / ordinary prose");
        let workbook = Workbook::new("w", "w")
            .with_sheet(sheet)
            .with_document(document);

        let cell = workbook.get_sheet("main").unwrap().get_cell(Coord::new(0, 0)).unwrap();
        assert_eq!(cell.document_refs, vec!["analysis#results"]);
        assert_eq!(workbook.documents_linked_from_cell("main", Coord::new(0, 0)).len(), 1);
        assert_eq!(workbook.documents_referencing_cell("main", Coord::new(0, 0)).len(), 1);
        assert_eq!(workbook.documents[0].referenced_cells(), vec!["answer", "A1"]);
    }

    #[test]
    fn qualified_document_cell_references_resolve_across_sheets() {
        let main = Sheet::new("main", "Main");
        let mut detail = Sheet::new("detail", "Details");
        detail.set_cell(Cell::number(Coord::new(0, 0), 7.0).with_name("detail_value"));
        let document = Document::new("analysis", "Analysis", "Detail: {{cell:detail!detail_value}}");
        let workbook = Workbook::new("w", "w")
            .with_sheet(main)
            .with_sheet(detail)
            .with_document(document);

        let rendered = workbook.documents[0].render_live(workbook.primary_sheet().unwrap(), Some(&workbook));
        assert_eq!(rendered, "Detail: 7.00");
        assert_eq!(workbook.documents_referencing_cell("detail", Coord::new(0, 0)).len(), 1);
        assert!(workbook.documents_referencing_cell("main", Coord::new(0, 0)).is_empty());
    }
}
