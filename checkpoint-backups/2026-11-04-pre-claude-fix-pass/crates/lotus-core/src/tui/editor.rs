//! Interactive spreadsheet workspace session and cursor state machine.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String};

use super::menu::MenuState;
use crate::{
    calc::CalcEngine,
    model::{Cell, CellValue, Coord, Sheet, TableOfContents},
};

pub struct WorkspaceSession {
    pub cursor: Coord,
    pub edit_buffer: Option<String>,
    pub menu: MenuState,
    pub status_message: String,
}

impl WorkspaceSession {
    pub fn new() -> Self {
        Self {
            cursor: Coord::new(0, 0),
            edit_buffer: None,
            menu: MenuState::new(),
            status_message: "READY (Press '/' for Lotus Menu, Enter to edit cell)".into(),
        }
    }

    pub fn move_cursor(&mut self, delta_col: i32, delta_row: i32) {
        if self.edit_buffer.is_some() || self.menu.is_active {
            return;
        }

        let new_col = (self.cursor.col as i64 + i64::from(delta_col)).clamp(0, 16_383) as u32;
        let new_row = (self.cursor.row as i64 + i64::from(delta_row)).clamp(0, 1_048_575) as u32;
        self.cursor = Coord::new(new_col, new_row);
    }

    pub fn start_editing(&mut self, sheet: &Sheet) {
        if self.menu.is_active {
            return;
        }

        let current_text = match sheet.get_cell(self.cursor) {
            Some(cell) => match &cell.value {
                CellValue::Formula { expr, .. } => expr.clone(),
                CellValue::Number(n) => format!("{}", n),
                CellValue::Text(t) => t.clone(),
                CellValue::Boolean(b) => (if *b { "TRUE" } else { "FALSE" }).into(),
                _ => String::new(),
            },
            None => String::new(),
        };

        self.edit_buffer = Some(current_text);
        self.status_message = "EDIT (Enter to commit, Esc to cancel)".into();
    }

    pub fn cancel_editing(&mut self) {
        self.edit_buffer = None;
        self.status_message = "READY".into();
    }

    pub fn commit_editing(&mut self, sheet: &mut Sheet) {
        if let Some(text) = self.edit_buffer.take() {
            let text = text.trim();
            if text.is_empty() {
                sheet.cells.remove(&self.cursor);
            } else {
                let cell_val = if text.starts_with('=') {
                    CellValue::Formula {
                        expr: text.into(),
                        cached_num: None,
                        cached_text: None,
                    }
                } else if let Ok(n) = text.parse::<f64>() {
                    CellValue::Number(n)
                } else if text.eq_ignore_ascii_case("true") {
                    CellValue::Boolean(true)
                } else if text.eq_ignore_ascii_case("false") {
                    CellValue::Boolean(false)
                } else {
                    CellValue::Text(text.into())
                };

                if let Some(cell) = sheet.cells.get_mut(&self.cursor) {
                    cell.value = cell_val;
                } else {
                    sheet.set_cell(Cell::new(self.cursor, cell_val));
                }
            }

            // Recalculate sheet
            let _ = CalcEngine::recalculate_sheet(sheet, None);
            self.status_message = format!("Cell {} updated.", self.cursor.to_a1());
        }
    }

    /// Renders the complete terminal workspace view including status bar,
    /// Lotus menu (or edit bar), and the active cell grid.
    pub fn render_workspace(&self, sheet: &Sheet, toc: &TableOfContents) -> String {
        self.render_workspace_with_viewport(sheet, toc, 6, 12)
    }

    /// Render the workspace using the supplied number of visible grid rows and columns.
    pub fn render_workspace_with_viewport(
        &self,
        sheet: &Sheet,
        toc: &TableOfContents,
        viewport_cols: u32,
        viewport_rows: u32,
    ) -> String {
        let mut screen = String::new();

        // Line 1: Cell Address & Current Content
        let cell_info = match sheet.get_cell(self.cursor) {
            Some(cell) => {
                let name = cell.name.as_deref().map(|n| format!(" ({})", n)).unwrap_or_default();
                let desc = match &cell.value {
                    CellValue::Formula { expr, cached_num, cached_text } => {
                        let cached = cached_num.map(|n| format!("{:.2}", n)).or_else(|| cached_text.clone()).unwrap_or_else(|| "<pending>".into());
                        format!("Formula: {} => {}", expr, cached)
                    }
                    CellValue::Number(n) => format!("Number: {:.2}", n),
                    CellValue::Text(t) => format!("Text: \"{}\"", t),
                    CellValue::Boolean(b) => format!("Boolean: {}", b),
                    CellValue::Basic { .. } => "Embedded BASIC Procedure".into(),
                    CellValue::Blob { bytes } => format!("Raw Binary Payload ({} bytes)", bytes.len()),
                    CellValue::Empty => "Empty".into(),
                    CellValue::Error(e) => format!("Error: {}", e),
                };
                format!("{}{}", desc, name)
            }
            None => "Empty".into(),
        };

        screen.push_str(&format!("{} | {:<6} {}\n", sheet.name, self.cursor.to_a1(), cell_info));

        // Line 2 & Line 3: Menu Bar or Edit Buffer
        if self.menu.is_active {
            let (l1, l2) = self.menu.render_lines();
            screen.push_str(&format!("MENU: {}\n", l1));
            screen.push_str(&format!("      {}\n", l2));
        } else if let Some(ref buf) = self.edit_buffer {
            screen.push_str(&format!("EDIT: {}\n\n", buf));
        } else {
            screen.push_str(&format!("STATUS: {}\n\n", self.status_message));
        }

        // Fixed grid window follows the cursor without drawing all cells from A1.
        screen.push_str(&crate::view::GridRenderer::render_viewport(
            sheet,
            Some(toc),
            self.cursor,
            viewport_cols,
            viewport_rows,
        ));

        screen
    }
}
