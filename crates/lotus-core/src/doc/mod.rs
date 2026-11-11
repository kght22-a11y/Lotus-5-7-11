//! Embedded Documents, Cross-References, and Live Templates.
//!
//! As specified in Lotus 5-7-11 Section 6:
//! - Documents (Markdown notes, specifications, reports) are first-class workbook objects.
//! - Cell-to-document references allow cells to link to supporting context and section anchors (`doc#fragment`).
//! - Document-to-cell references embed live spreadsheet values (e.g. `{{cell:revenue}}` or `{{cell:A1}}`)
//!   or immutable point-in-time snapshots (`{{snapshot:A1}}`).

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, string::ToString, vec, vec::Vec};

use crate::model::{Cell, Coord, Sheet, Workbook};

/// Representation of a parsed Markdown section heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionHeading {
    pub level: usize,
    pub title: String,
    pub anchor: String,
    pub line_index: usize,
}

/// Convert a heading title into a URL/anchor slug:
/// - Lowercase ASCII
/// - Alphanumerics retained
/// - Spaces, underscores, slashes, and hyphens collapsed into single dashes
/// - Leading and trailing dashes trimmed
pub fn heading_to_slug(title: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if ch == ' ' || ch == '-' || ch == '_' || ch == '/' {
            if !prev_dash && !slug.is_empty() {
                slug.push('-');
                prev_dash = true;
            }
        }
    }
    if slug.ends_with('-') {
        slug.pop();
    }
    slug
}

/// Parse a line to check if it represents a Markdown heading (1 to 6 `#` characters followed by space/tab/EOL).
pub fn parse_heading_line(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }

    let mut level = 0;
    let chars: Vec<char> = trimmed.chars().collect();
    while level < chars.len() && chars[level] == '#' && level < 6 {
        level += 1;
    }

    if level == 0 {
        return None;
    }

    if level < chars.len() && chars[level] != ' ' && chars[level] != '\t' {
        return None;
    }

    let title: String = chars[level..].iter().collect();
    Some((level, title.trim().to_string()))
}

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
    pub fn render_live(&self, sheet: &Sheet, workbook: Option<&Workbook>) -> String {
        render_template_str(&self.body, sheet, workbook)
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

    /// Extract all Markdown section headings from the document body.
    pub fn headings(&self) -> Vec<SectionHeading> {
        let mut headings = Vec::new();
        for (line_index, line) in self.body.lines().enumerate() {
            if let Some((level, title)) = parse_heading_line(line) {
                let anchor = heading_to_slug(&title);
                headings.push(SectionHeading {
                    level,
                    title,
                    anchor,
                    line_index,
                });
            }
        }
        headings
    }

    /// Find a section heading matching a fragment (by canonical slug, case-insensitive anchor, or title).
    pub fn find_section(&self, fragment: &str) -> Option<SectionHeading> {
        let clean_frag = fragment.strip_prefix('#').unwrap_or(fragment).trim();
        if clean_frag.is_empty() {
            return None;
        }
        let frag_slug = heading_to_slug(clean_frag);

        self.headings().into_iter().find(|h| {
            h.anchor == clean_frag.to_ascii_lowercase()
                || h.anchor == frag_slug
                || h.title.eq_ignore_ascii_case(clean_frag)
        })
    }

    /// Check whether a section anchor/heading exists in this document.
    pub fn has_section(&self, fragment: &str) -> bool {
        self.find_section(fragment).is_some()
    }

    /// Extract the unrendered body text of a specific section (from the matching heading
    /// up to the next heading of equal or higher level, or end of document).
    pub fn section_content(&self, fragment: &str) -> Option<String> {
        let target = self.find_section(fragment)?;
        let lines: Vec<&str> = self.body.lines().collect();
        let mut section_lines = Vec::new();

        for (idx, line) in lines.iter().enumerate().skip(target.line_index) {
            if idx > target.line_index {
                if let Some((level, _)) = parse_heading_line(line) {
                    if level <= target.level {
                        break;
                    }
                }
            }
            section_lines.push(*line);
        }

        Some(section_lines.join("\n"))
    }

    /// Render a specific section of the document with live cell references resolved.
    pub fn render_section(&self, fragment: &str, sheet: &Sheet, workbook: Option<&Workbook>) -> Option<String> {
        let content = self.section_content(fragment)?;
        Some(render_template_str(&content, sheet, workbook))
    }
}

/// Render live cell templates in arbitrary text against the given sheet and workbook.
pub fn render_template_str(text: &str, sheet: &Sheet, workbook: Option<&Workbook>) -> String {
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
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
                let resolved = resolve_tag(tag.trim(), sheet, workbook);
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

/// Status of an outgoing document reference (including optional section fragment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentRefStatus {
    /// Document exists and no section fragment was specified.
    Valid,
    /// Document exists and the section fragment was resolved to a valid heading.
    ValidWithSection(String),
    /// The referenced document does not exist in the workbook.
    UnresolvedDocument,
    /// The document exists, but the section fragment could not be found.
    UnresolvedSection(String),
}

/// Broken outgoing document link from a worksheet cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenDocRef {
    pub sheet_id: String,
    pub coord: Coord,
    pub cell_name: Option<String>,
    pub target: String,
    pub reason: String,
}

/// Broken section anchor in an outgoing document link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenSectionRef {
    pub sheet_id: String,
    pub coord: Coord,
    pub cell_name: Option<String>,
    pub document_id: String,
    pub section_anchor: String,
    pub target: String,
}

/// Broken cell reference embedded within a document template (`{{cell:...}}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenDocCellRef {
    pub document_id: String,
    pub target: String,
    pub reason: String,
}

/// Dangling formula reference pointing to an empty or non-existent cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DanglingFormulaRef {
    pub sheet_id: String,
    pub coord: Coord,
    pub formula_name: Option<String>,
    pub target: String,
}

/// Comprehensive report of orphaned references and missing links across the workbook.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OrphanReport {
    pub broken_document_refs: Vec<BrokenDocRef>,
    pub broken_section_refs: Vec<BrokenSectionRef>,
    pub broken_document_cell_refs: Vec<BrokenDocCellRef>,
    pub unreferenced_documents: Vec<String>,
    pub dangling_formula_refs: Vec<DanglingFormulaRef>,
}

impl OrphanReport {
    pub fn is_clean(&self) -> bool {
        self.broken_document_refs.is_empty()
            && self.broken_section_refs.is_empty()
            && self.broken_document_cell_refs.is_empty()
            && self.unreferenced_documents.is_empty()
            && self.dangling_formula_refs.is_empty()
    }

    pub fn total_issues(&self) -> usize {
        self.broken_document_refs.len()
            + self.broken_section_refs.len()
            + self.broken_document_cell_refs.len()
            + self.unreferenced_documents.len()
            + self.dangling_formula_refs.len()
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

    #[test]
    fn markdown_headings_are_parsed_and_slugified() {
        let body = r#"# Main Title
Introductory text.

## 1. Inputs & Parameters
Some parameters.

### Detailed Rate Table
Details here.

## Summary
The end.
"#;
        let doc = Document::new("test", "Test Doc", body);
        let headings = doc.headings();
        assert_eq!(headings.len(), 4);
        assert_eq!(headings[0].title, "Main Title");
        assert_eq!(headings[0].anchor, "main-title");
        assert_eq!(headings[0].level, 1);

        assert_eq!(headings[1].title, "1. Inputs & Parameters");
        assert_eq!(headings[1].anchor, "1-inputs-parameters");
        assert_eq!(headings[1].level, 2);

        assert_eq!(headings[2].title, "Detailed Rate Table");
        assert_eq!(headings[2].anchor, "detailed-rate-table");
        assert_eq!(headings[2].level, 3);

        assert_eq!(headings[3].title, "Summary");
        assert_eq!(headings[3].anchor, "summary");
        assert_eq!(headings[3].level, 2);
    }

    #[test]
    fn markdown_section_content_and_live_rendering() {
        let mut sheet = Sheet::new("main", "Main");
        sheet.set_cell(Cell::number(Coord::new(0, 0), 5000.0).with_name("revenue"));
        sheet.set_cell(Cell::number(Coord::new(0, 1), 0.10).with_name("rate"));

        let body = r#"# Overview
General notes.

## Section Alpha
Revenue is ${{cell:revenue}}.

## Section Beta
Rate is {{cell:rate}}.

# Conclusion
Final thoughts.
"#;
        let doc = Document::new("report", "Report", body);

        // Find by exact anchor
        let heading_alpha = doc.find_section("section-alpha").expect("section alpha found");
        assert_eq!(heading_alpha.title, "Section Alpha");

        // Find with leading #
        assert!(doc.has_section("#section-beta"));

        // Find by title case-insensitive
        assert!(doc.has_section("section beta"));

        // Extract section content
        let alpha_content = doc.section_content("section-alpha").unwrap();
        assert!(alpha_content.contains("Revenue is ${{cell:revenue}}."));
        assert!(!alpha_content.contains("Section Beta"));

        // Render live section
        let rendered_alpha = doc.render_section("section-alpha", &sheet, None).unwrap();
        assert!(rendered_alpha.contains("Revenue is $5000.00."));

        // Non-existent section
        assert!(doc.find_section("missing-section").is_none());
        assert!(doc.render_section("missing-section", &sheet, None).is_none());
    }
}
