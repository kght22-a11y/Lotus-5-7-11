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
    pub edit_cursor: usize,
    pub edit_selection_anchor: Option<usize>,
    pub menu: MenuState,
    pub status_message: String,
    pub is_dirty: bool,
}

#[cfg(test)]
mod tests {
    use super::WorkspaceSession;
    use crate::model::{Sheet, Workbook};

    #[test]
    fn dirty_workspace_marks_sheet_title() {
        let sheet = Sheet::new("main", "Main");
        let toc = Workbook::new("w", "w").with_sheet(sheet.clone()).compute_toc();
        let mut session = WorkspaceSession::new();
        session.is_dirty = true;
        assert!(session.render_workspace(&sheet, &toc).starts_with("Main* |"));
    }

    #[test]
    fn edit_buffer_is_visible_on_the_control_panel_edit_line() {
        let sheet = Sheet::new("main", "Main");
        let toc = Workbook::new("w", "w")
            .with_sheet(sheet.clone())
            .compute_toc();
        let mut session = WorkspaceSession::new();
        session.edit_buffer = Some("100".into());
        let rendered = session.render_workspace(&sheet, &toc);
        let mut lines = rendered.lines();
        assert!(lines.next().unwrap().starts_with("Main | A1"));
        assert_eq!(lines.next(), Some("EDIT: 100 | Enter commits, Esc cancels"));
    }

    #[test]
    fn test_edit_selection_and_cursor_movement() {
        let mut session = WorkspaceSession::new();
        session.edit_buffer = Some("HELLO WORLD".into());
        session.edit_cursor = 11; // at end

        // Shift+Left 5 times selects "WORLD"
        for _ in 0..5 {
            session.move_edit_cursor(-1, true);
        }
        assert_eq!(session.edit_cursor, 6);
        assert_eq!(session.edit_selection_anchor, Some(11));
        assert_eq!(session.selection_range(), Some((6, 11)));
        assert_eq!(session.selected_text().as_deref(), Some("WORLD"));

        // Typing a character replaces selection
        session.insert_char('!');
        assert_eq!(session.edit_buffer.as_deref(), Some("HELLO !"));
        assert_eq!(session.edit_cursor, 7);
        assert_eq!(session.selection_range(), None);
    }
}

impl WorkspaceSession {
    pub fn new() -> Self {
        Self {
            cursor: Coord::new(0, 0),
            edit_buffer: None,
            edit_cursor: 0,
            edit_selection_anchor: None,
            menu: MenuState::new(),
            status_message: "READY (Press '/' for Lotus Menu, Enter to edit cell)".into(),
            is_dirty: false,
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

        self.edit_cursor = current_text.len();
        self.edit_selection_anchor = None;
        self.edit_buffer = Some(current_text);
        self.status_message = "EDIT (Enter to commit, Esc to cancel)".into();
    }

    pub fn cancel_editing(&mut self) {
        self.edit_buffer = None;
        self.edit_cursor = 0;
        self.edit_selection_anchor = None;
        self.status_message = "READY".into();
    }

    pub fn commit_editing(&mut self, sheet: &mut Sheet) {
        self.edit_cursor = 0;
        self.edit_selection_anchor = None;
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
                        cached_bool: None,
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

    /// Returns the active selection byte range `(start, end)` if any text is selected.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.edit_selection_anchor?;
        let cursor = self.edit_cursor;
        if anchor == cursor {
            None
        } else {
            Some((anchor.min(cursor), anchor.max(cursor)))
        }
    }

    /// Returns the currently selected text substring if any.
    pub fn selected_text(&self) -> Option<String> {
        let buf = self.edit_buffer.as_ref()?;
        let (start, end) = self.selection_range()?;
        if start <= end && end <= buf.len() {
            Some(buf[start..end].to_string())
        } else {
            None
        }
    }

    /// Deletes the currently selected text range. Returns true if text was deleted.
    pub fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection_range() {
            if let Some(ref mut buf) = self.edit_buffer {
                if start <= end && end <= buf.len() {
                    buf.replace_range(start..end, "");
                    self.edit_cursor = start;
                    self.edit_selection_anchor = None;
                    return true;
                }
            }
        }
        false
    }

    /// Insert a character into the edit buffer at the current cursor, replacing any selection.
    pub fn insert_char(&mut self, ch: char) {
        self.delete_selection();
        if let Some(ref mut buf) = self.edit_buffer {
            let idx = self.edit_cursor.min(buf.len());
            buf.insert(idx, ch);
            self.edit_cursor = idx + ch.len_utf8();
            self.edit_selection_anchor = None;
        } else {
            self.edit_buffer = Some(ch.to_string());
            self.edit_cursor = ch.len_utf8();
            self.edit_selection_anchor = None;
        }
    }

    /// Insert a string into the edit buffer, replacing any selection.
    pub fn insert_str(&mut self, s: &str) {
        self.delete_selection();
        if let Some(ref mut buf) = self.edit_buffer {
            let idx = self.edit_cursor.min(buf.len());
            buf.insert_str(idx, s);
            self.edit_cursor = idx + s.len();
            self.edit_selection_anchor = None;
        } else {
            self.edit_buffer = Some(s.into());
            self.edit_cursor = s.len();
            self.edit_selection_anchor = None;
        }
    }

    /// Backspace key: deletes selection, or character before cursor.
    pub fn backspace(&mut self) {
        if !self.delete_selection() {
            if let Some(ref mut buf) = self.edit_buffer {
                if self.edit_cursor > 0 && self.edit_cursor <= buf.len() {
                    let mut prev_idx = self.edit_cursor - 1;
                    while !buf.is_char_boundary(prev_idx) && prev_idx > 0 {
                        prev_idx -= 1;
                    }
                    buf.drain(prev_idx..self.edit_cursor);
                    self.edit_cursor = prev_idx;
                }
            }
        }
    }

    /// Delete key: deletes selection, or character after cursor.
    pub fn delete_forward(&mut self) {
        if !self.delete_selection() {
            if let Some(ref mut buf) = self.edit_buffer {
                if self.edit_cursor < buf.len() {
                    let mut next_idx = self.edit_cursor + 1;
                    while !buf.is_char_boundary(next_idx) && next_idx < buf.len() {
                        next_idx += 1;
                    }
                    buf.drain(self.edit_cursor..next_idx);
                }
            }
        }
    }

    /// Move the edit cursor left or right. If `select` is true (Shift+Arrow), extends selection!
    pub fn move_edit_cursor(&mut self, delta: i32, select: bool) {
        if let Some(ref buf) = self.edit_buffer {
            let len = buf.len();
            if select && self.edit_selection_anchor.is_none() {
                self.edit_selection_anchor = Some(self.edit_cursor);
            } else if !select {
                self.edit_selection_anchor = None;
            }

            if delta < 0 {
                let mut new_pos = self.edit_cursor;
                for _ in 0..(-delta) {
                    if new_pos > 0 {
                        new_pos -= 1;
                        while !buf.is_char_boundary(new_pos) && new_pos > 0 {
                            new_pos -= 1;
                        }
                    }
                }
                self.edit_cursor = new_pos;
            } else if delta > 0 {
                let mut new_pos = self.edit_cursor;
                for _ in 0..delta {
                    if new_pos < len {
                        new_pos += 1;
                        while !buf.is_char_boundary(new_pos) && new_pos < len {
                            new_pos += 1;
                        }
                    }
                }
                self.edit_cursor = new_pos;
            }
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
                    CellValue::Formula { expr, cached_num, cached_text, cached_bool } => {
                        let cached = cached_num.map(|n| format!("{:.2}", n))
                            .or_else(|| cached_bool.map(|value| if value { "TRUE".into() } else { "FALSE".into() }))
                            .or_else(|| cached_text.clone()).unwrap_or_else(|| "<pending>".into());
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
                let links = if !cell.document_refs.is_empty() {
                    format!(" [Links: {}]", cell.document_refs.join(", "))
                } else {
                    String::new()
                };
                format!("{}{}{}", desc, name, links)
            }
            None => "Empty".into(),
        };

        screen.push_str(&format!("{}{} | {:<6} {}\n", sheet.name, if self.is_dirty { "*" } else { "" }, self.cursor.to_a1(), cell_info));

        // Line 2 & Line 3: Menu Bar or Edit Buffer
        if self.menu.is_active {
            let (l1, l2) = self.menu.render_lines();
            screen.push_str(&format!("MENU: {}\n", l1));
            screen.push_str(&format!("      {}\n", l2));
        } else if self.edit_buffer.is_some() {
            screen.push_str(&format!(
                "EDIT: {} | Enter commits, Esc cancels\n\n",
                self.edit_buffer.as_deref().unwrap_or_default()
            ));
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
