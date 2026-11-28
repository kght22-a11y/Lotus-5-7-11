//! Interactive spreadsheet workspace session and cursor state machine.

#[cfg(not(feature = "std"))]
use alloc::{format, string::{String, ToString}};

use super::menu::MenuState;
use crate::{
    calc::CalcEngine,
    model::{Cell, CellValue, Coord, Sheet, TableOfContents},
    ops::Range,
};

pub struct WorkspaceSession {
    pub cursor: Coord,
    pub edit_buffer: Option<String>,
    pub edit_cursor: usize,
    pub edit_selection_anchor: Option<usize>,
    /// When the edit line is answering a question (Find, Go to, column width)
    /// instead of editing a cell, the question shown in front of it.
    pub edit_prompt: Option<String>,
    /// The other corner of a multi-cell selection; the cursor is the first corner.
    pub selection_anchor: Option<Coord>,
    /// Extend mode (F8): arrow keys grow the selection instead of clearing it.
    pub extend_mode: bool,
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
            edit_prompt: None,
            selection_anchor: None,
            extend_mode: false,
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
        self.edit_prompt = None;
        self.status_message = "READY".into();
    }

    pub fn commit_editing(&mut self, sheet: &mut Sheet) {
        self.edit_cursor = 0;
        self.edit_selection_anchor = None;
        if let Some(text) = self.edit_buffer.take() {
            let cell_val = CellValue::parse_input(&text);
            if cell_val.is_empty() {
                sheet.cells.remove(&self.cursor);
            } else if let Some(cell) = sheet.cells.get_mut(&self.cursor) {
                cell.value = cell_val;
            } else {
                sheet.set_cell(Cell::new(self.cursor, cell_val));
            }

            // Recalculate sheet
            let _ = CalcEngine::recalculate_sheet(sheet, None);
            self.status_message = format!("Cell {} updated.", self.cursor.to_a1());
        }
    }

    /// The selected cells, if more than one cell is selected.
    pub fn selection(&self) -> Option<Range> {
        let anchor = self.selection_anchor?;
        if anchor == self.cursor { None } else { Some(Range::new(anchor, self.cursor)) }
    }

    /// The selected cells, or just the cursor cell when nothing else is selected.
    pub fn selected_range(&self) -> Range {
        self.selection().unwrap_or_else(|| Range::cell(self.cursor))
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.extend_mode = false;
    }

    /// Start typing into an empty edit line (typing over a cell replaces it).
    pub fn begin_blank_edit(&mut self) {
        self.edit_buffer = Some(String::new());
        self.edit_cursor = 0;
        self.edit_selection_anchor = None;
        self.edit_prompt = None;
        self.status_message = "EDIT (Enter to commit, Esc to cancel)".into();
    }

    /// Use the edit line to ask a question; the answer comes back through [`end_prompt`](Self::end_prompt).
    pub fn begin_prompt(&mut self, label: &str) {
        self.begin_blank_edit();
        self.edit_prompt = Some(label.into());
        self.status_message = format!("{} (Enter to accept, Esc to cancel)", label.trim_end_matches([':', ' ']));
    }

    /// Finish a prompt and return what was typed.
    pub fn end_prompt(&mut self) -> Option<String> {
        self.edit_prompt = None;
        self.edit_cursor = 0;
        self.edit_selection_anchor = None;
        self.edit_buffer.take()
    }

    /// Move the edit caret to `pos` (a byte index on a character boundary),
    /// extending the text selection when `select` is held.
    fn set_edit_cursor(&mut self, pos: usize, select: bool) {
        if select {
            if self.edit_selection_anchor.is_none() {
                self.edit_selection_anchor = Some(self.edit_cursor);
            }
        } else {
            self.edit_selection_anchor = None;
        }
        self.edit_cursor = pos;
    }

    /// Home: caret to the start of the edit line.
    pub fn edit_home(&mut self, select: bool) {
        if self.edit_buffer.is_some() {
            self.set_edit_cursor(0, select);
        }
    }

    /// End: caret to the end of the edit line.
    pub fn edit_end(&mut self, select: bool) {
        if let Some(len) = self.edit_buffer.as_ref().map(|b| b.len()) {
            self.set_edit_cursor(len, select);
        }
    }

    /// Select the whole edit line.
    pub fn edit_select_all(&mut self) {
        if let Some(len) = self.edit_buffer.as_ref().map(|b| b.len()) {
            self.edit_selection_anchor = Some(0);
            self.edit_cursor = len;
        }
    }

    /// Ctrl+Left / Ctrl+Right: move the caret a word at a time.
    pub fn move_edit_word(&mut self, direction: i32, select: bool) {
        let Some(buf) = self.edit_buffer.as_ref() else { return };
        let pos = self.edit_cursor.min(buf.len());
        let target = if direction < 0 {
            let before = &buf[..pos];
            let trimmed = before.trim_end().len();
            before[..trimmed].rfind(char::is_whitespace).map_or(0, |i| {
                i + before[i..].chars().next().map_or(1, char::len_utf8)
            })
        } else {
            let after = &buf[pos..];
            let word_end = after.find(char::is_whitespace).unwrap_or(after.len());
            let rest = &after[word_end..];
            pos + word_end + (rest.len() - rest.trim_start().len())
        };
        self.set_edit_cursor(target, select);
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

#[cfg(test)]
mod selection_and_prompt_tests {
    use super::WorkspaceSession;
    use crate::model::Coord;
    use crate::ops::Range;

    fn editing(text: &str, caret: usize) -> WorkspaceSession {
        let mut s = WorkspaceSession::new();
        s.edit_buffer = Some(text.into());
        s.edit_cursor = caret;
        s
    }

    #[test]
    fn a_cell_selection_is_the_cursor_plus_the_anchor() {
        let mut s = WorkspaceSession::new();
        s.cursor = Coord::new(2, 2);
        assert_eq!(s.selection(), None);
        assert_eq!(s.selected_range(), Range::cell(Coord::new(2, 2)));

        s.selection_anchor = Some(Coord::new(0, 0));
        assert_eq!(s.selection(), Some(Range::new(Coord::new(0, 0), Coord::new(2, 2))));
        assert_eq!(s.selected_range().to_a1(), "A1:C3");

        s.selection_anchor = Some(Coord::new(2, 2));
        assert_eq!(s.selection(), None, "an anchor on the cursor is not a range");

        s.extend_mode = true;
        s.clear_selection();
        assert_eq!((s.selection_anchor, s.extend_mode), (None, false));
    }

    #[test]
    fn home_end_and_select_all_move_and_select() {
        let mut s = editing("hello", 2);
        s.edit_end(true);
        assert_eq!((s.edit_cursor, s.selected_text().as_deref()), (5, Some("llo")));
        s.edit_home(false);
        assert_eq!((s.edit_cursor, s.selected_text()), (0, None), "moving without Shift drops the selection");
        s.edit_select_all();
        assert_eq!(s.selected_text().as_deref(), Some("hello"));

        let mut none = WorkspaceSession::new();
        none.edit_home(false); // not editing: nothing happens, nothing panics
        none.edit_end(true);
        none.edit_select_all();
    }

    #[test]
    fn word_movement_skips_runs_of_spaces() {
        let mut s = editing("one   two  three", 0);
        s.move_edit_word(1, false);
        assert_eq!(s.edit_cursor, 6, "to the start of 'two'");
        s.move_edit_word(1, false);
        assert_eq!(s.edit_cursor, 11, "to the start of 'three'");
        s.move_edit_word(1, false);
        assert_eq!(s.edit_cursor, 16, "to the end");
        s.move_edit_word(-1, false);
        assert_eq!(s.edit_cursor, 11);
        s.move_edit_word(-1, false);
        assert_eq!(s.edit_cursor, 6);
        s.move_edit_word(-1, false);
        assert_eq!(s.edit_cursor, 0);
        s.move_edit_word(-1, false);
        assert_eq!(s.edit_cursor, 0, "stays at the start");
    }

    #[test]
    fn word_movement_extends_a_selection_and_respects_multibyte_text() {
        let mut s = editing("héllo wörld", 0);
        s.move_edit_word(1, true);
        assert_eq!(s.selected_text().as_deref(), Some("héllo "));
        s.move_edit_word(1, true);
        assert_eq!(s.selected_text().as_deref(), Some("héllo wörld"));
        s.move_edit_word(-1, false);
        assert_eq!(s.edit_cursor, "héllo ".len(), "lands on a character boundary");
    }

    #[test]
    fn a_prompt_collects_an_answer_and_cleans_up() {
        let mut s = WorkspaceSession::new();
        s.begin_prompt("Find: ");
        assert_eq!(s.edit_prompt.as_deref(), Some("Find: "));
        assert_eq!(s.edit_buffer.as_deref(), Some(""));
        s.insert_str("abc");
        assert_eq!(s.end_prompt().as_deref(), Some("abc"));
        assert_eq!((s.edit_prompt.clone(), s.edit_buffer.clone(), s.edit_cursor), (None, None, 0));
    }

    #[test]
    fn cancelling_drops_the_prompt_too() {
        let mut s = WorkspaceSession::new();
        s.begin_prompt("Go to: ");
        s.cancel_editing();
        assert_eq!((s.edit_prompt, s.edit_buffer), (None, None));
    }

    #[test]
    fn typing_over_a_blank_edit_starts_clean() {
        let mut s = editing("old", 3);
        s.edit_selection_anchor = Some(0);
        s.begin_blank_edit();
        assert_eq!((s.edit_buffer.as_deref(), s.edit_cursor, s.edit_selection_anchor), (Some(""), 0, None));
    }
}
