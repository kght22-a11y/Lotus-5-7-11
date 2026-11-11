//! Embedded Documents, Cross-References, and Live Templates.
//!
//! As specified in Lotus 5-7-11 Section 6:
//! - Documents (Markdown notes, specifications, reports) are first-class workbook objects.
//! - Cell-to-document references allow cells to link to supporting context.
//! - Document-to-cell references embed live spreadsheet values (e.g. `{{cell:revenue}}` or `{{cell:A1}}`)
//!   or immutable point-in-time snapshots (`{{snapshot:A1}}`).

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, string::ToString, vec::Vec};

use crate::model::{Coord, Sheet, Workbook};

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
                    let resolved = resolve_tag(tag.trim(), sheet);
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
}

fn resolve_tag(tag: &str, sheet: &Sheet) -> String {
    if let Some(target) = tag.strip_prefix("cell:") {
        let target = target.trim();
        // Try coordinate first
        let coord = if let Some(c) = Coord::parse_a1(target) {
            Some(c)
        } else {
            // Try cell name
            sheet.cells.values().find(|c| c.name.as_deref() == Some(target)).map(|c| c.coord)
        };

        if let Some(c) = coord {
            if let Some(cell) = sheet.get_cell(c) {
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
}
