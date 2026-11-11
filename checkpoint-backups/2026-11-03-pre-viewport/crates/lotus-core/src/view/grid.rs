//! Terminal grid rasterizer and visual spreadsheet formatter.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec, vec::Vec};

use crate::model::{CellValue, Coord, Sheet, TableOfContents};

pub struct GridRenderer;

impl GridRenderer {
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
