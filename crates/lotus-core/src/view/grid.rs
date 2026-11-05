//! Terminal grid rasterizer and visual spreadsheet formatter.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec, vec::Vec};

use crate::model::{CellValue, Coord, Sheet, TableOfContents};

pub struct GridRenderer;

impl GridRenderer {
    /// Render a fixed-size window around the active cell.
    pub fn render_viewport(
        sheet: &Sheet,
        toc: Option<&TableOfContents>,
        cursor: Coord,
        viewport_cols: u32,
        viewport_rows: u32,
    ) -> String {
        let cols = viewport_cols.max(1);
        let rows = viewport_rows.max(1);
        let start_col = cursor.col.saturating_sub(cols / 2);
        let start_row = cursor.row.saturating_sub(rows / 2);
        let row_header_width = format!("{}", start_row.saturating_add(rows)).len().max(3) + 1;
        // Fixed-width cells keep the viewport within the terminal width; long values
        // are truncated in-grid while the status line shows the selected cell in full.
        let inner_widths = vec![10usize; cols as usize];

        let mut out = String::new();
        out.push_str(&format!("{:>width$} │", "", width = row_header_width));
        for offset in 0..cols {
            let col = start_col.saturating_add(offset);
            let label: String = Coord::new(col, 0).to_a1().chars().filter(|c| c.is_alphabetic()).collect();
            let width = inner_widths[offset as usize] + 2;
            out.push_str(&format!(" {:^width$} │", label, width = width));
        }
        out.push('\n');
        out.push_str(&format!("{:─>width$}─┼", "", width = row_header_width));
        for width in &inner_widths {
            out.push_str(&format!("{:─>width$}─┼", "", width = width + 2));
        }
        out.push('\n');

        for row_offset in 0..rows {
            let row = start_row.saturating_add(row_offset);
            let marker = if row == cursor.row { ">" } else { " " };
            let row_label = format!("{}{}", marker, row + 1);
            out.push_str(&format!("{:>width$} │", row_label, width = row_header_width));

            for col_offset in 0..cols {
                let col = start_col.saturating_add(col_offset);
                let coord = Coord::new(col, row);
                let inner_width = inner_widths[col_offset as usize];
                let width = inner_width + 2;
                let is_toc = toc.is_some_and(|table| table.pos == coord);
                if is_toc {
                    out.push_str(&format!(" {:^width$} │", "«TOC»", width = width));
                } else if let Some(cell) = sheet.get_cell(coord) {
                    let display = fit_cell(&cell_display_string(&cell.value), inner_width);
                    if coord == cursor {
                        out.push_str(&format!(" [{:<width$}] │", display, width = inner_width));
                    } else {
                        out.push_str(&format!(" {:<width$} │", display, width = width));
                    }
                } else if coord == cursor {
                    out.push_str(&format!(" [{:width$}] │", "", width = inner_width));
                } else {
                    out.push_str(&format!(" {:width$} │", "", width = width));
                }
            }
            out.push('\n');
        }
        out
    }

    /// Renders a worksheet as a formatted ASCII/Unicode terminal grid.
    pub fn render_sheet(sheet: &Sheet, toc: Option<&TableOfContents>, max_display_cols: Option<u32>, max_display_rows: Option<u32>) -> String {
        let (actual_max_col, actual_max_row) = sheet.bounds().unwrap_or((0, 0));
        let toc_col = toc.map(|t| t.pos.col).unwrap_or(0);
        let toc_row = toc.map(|t| t.pos.row).unwrap_or(0);

        let max_col = max_display_cols
            .unwrap_or(actual_max_col.max(4))
            .max(actual_max_col)
            .max(toc_col);
        let max_row = max_display_rows
            .unwrap_or(actual_max_row.max(9))
            .max(actual_max_row)
            .max(toc_row);

        // Determine column widths (default 12 chars)
        let mut col_widths: Vec<usize> = vec![12; (max_col + 1) as usize];

        // Header width for row numbers
        let row_num_width = format!("{}", max_row + 1).len().max(3);

        // Check if any cell content requires wider columns
        for cell in sheet.cells.values() {
            let col = cell.coord.col as usize;
            if col < col_widths.len() {
                let text = cell_display_string(&cell.value);
                if text.len() + 2 > col_widths[col] {
                    col_widths[col] = (text.len() + 2).min(30);
                }
            }
        }

        let mut out = String::new();

        // 1. Column Headers (A, B, C...)
        out.push_str(&format!("{:>width$} │", "", width = row_num_width));
        for col in 0..=max_col {
            let col_name = Coord::new(col, 0).to_a1();
            let col_letter: String = col_name.chars().filter(|c| c.is_alphabetic()).collect();
            let w = col_widths[col as usize];
            out.push_str(&format!(" {:^width$} │", col_letter, width = w));
        }
        out.push('\n');

        // Divider
        out.push_str(&format!("{:─>width$}─┼", "", width = row_num_width));
        for col in 0..=max_col {
            let w = col_widths[col as usize];
            out.push_str(&format!("{:─>width$}─┼", "", width = w));
        }
        out.push('\n');

        // 2. Rows
        for row in 0..=max_row {
            out.push_str(&format!("{:>width$} │", row + 1, width = row_num_width));

            for col in 0..=max_col {
                let coord = Coord::new(col, row);
                let w = col_widths[col as usize];

                let is_toc = toc.map(|t| t.pos == coord).unwrap_or(false);

                if is_toc {
                    out.push_str(&format!(" {:^width$} │", "«TOC»", width = w));
                } else if let Some(cell) = sheet.get_cell(coord) {
                    let display = cell_display_string(&cell.value);
                    let is_numeric = match &cell.value {
                        CellValue::Number(_) => true,
                        CellValue::Formula { cached_num: Some(_), .. } => true,
                        _ => false,
                    };

                    if is_numeric {
                        out.push_str(&format!(" {:>width$} │", display, width = w));
                    } else {
                        out.push_str(&format!(" {:<width$} │", display, width = w));
                    }
                } else {
                    out.push_str(&format!(" {:width$} │", "", width = w));
                }
            }
            out.push('\n');
        }

        out
    }
}

fn fit_cell(value: &str, width: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= width {
        return value.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let mut fitted: String = value.chars().take(width - 1).collect();
    fitted.push('…');
    fitted
}

fn cell_display_string(val: &CellValue) -> String {
    match val {
        CellValue::Empty => String::new(),
        CellValue::Number(n) => format!("{:.2}", n),
        CellValue::Text(t) => t.clone(),
        CellValue::Boolean(b) => (if *b { "TRUE" } else { "FALSE" }).into(),
        CellValue::Formula { cached_num, cached_text, expr } => {
            if let Some(n) = cached_num {
                format!("{:.2}", n)
            } else if let Some(ref t) = cached_text {
                t.clone()
            } else {
                format!("={}", expr)
            }
        }
        CellValue::Basic { .. } => "[BASIC]".into(),
        CellValue::Blob { bytes } => format!("[BLOB:{}B]", bytes.len()),
        CellValue::Error(e) => e.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Cell;

    #[test]
    fn viewport_stays_fixed_and_centers_remote_cursor() {
        let mut sheet = Sheet::new("main", "Main");
        let cursor = Coord::new(100, 500);
        sheet.set_cell(Cell::text(cursor, "active"));

        let rendered = GridRenderer::render_viewport(&sheet, None, cursor, 4, 4);
        assert_eq!(rendered.lines().count(), 6);
        assert!(rendered.contains("CW"));
        assert!(rendered.contains("[active"));
        assert!(rendered.contains(">501"));
    }
}
