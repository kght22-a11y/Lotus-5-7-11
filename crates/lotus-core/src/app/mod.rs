//! Portable application layer: everything the desktop GUI does except talk to the OS.
//!
//! [`App`] owns the workbook, the editing session, the menus and the pixel
//! buffer. A platform shell drives it with three calls:
//!
//! ```text
//! app.handle_event(&mut host, event);   // native input, translated to Event
//! if app.take_dirty() { /* ask the OS to repaint */ }
//! let frame = app.render();             // blit frame.pixels (0x00RRGGBB, top-down)
//! ```
//!
//! The only things a platform must supply are a surface to blit onto, the
//! input translation, and a [`Host`] (clipboard, whole-file read/write, quit).
//! Because `App` never calls the OS itself it runs unchanged under a test
//! with a [`MemoryHost`], under a Win32 window, or on bare metal.
//!
//! # Keys
//!
//! | Keys                    | Action                                              |
//! |-------------------------|-----------------------------------------------------|
//! | Arrows                  | move the cursor                                     |
//! | Shift+Arrows            | move the view                                       |
//! | Ctrl+Shift+Arrows       | resize the cursor's column (left/right) or row (up/down) |
//! | Ctrl+Arrows             | jump to the edge of the data                        |
//! | Home / End              | start / end of the row (Ctrl: A1 / last used cell)  |
//! | PageUp / PageDown       | a screenful up / down                               |
//! | Tab / Shift+Tab         | right / left                                        |
//! | F2, F3, F5, F8          | edit, find next, go to, extend selection            |
//! | Ctrl+C X V              | copy, cut, paste (ranges travel as tab-separated text) |
//! | Ctrl+Z / Ctrl+Y         | undo / redo                                         |
//! | Ctrl+A                  | select all                                          |
//! | Ctrl+D / Ctrl+R         | fill down / right                                   |
//! | Ctrl+F / Ctrl+G         | find / go to                                        |
//! | Ctrl+0 / Ctrl+9         | collapse column / row (add Shift to expand)         |
//! | Ctrl+S                  | save                                                |
//!
//! While a cell is being edited, the same keys act as a text editor: arrows,
//! Home/End, Ctrl+arrows (words), Shift to select, Ctrl+A/C/X/V.

#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap,
    format,
    string::String,
    vec::Vec,
};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

pub mod event;
pub mod host;

pub use event::{Event, Key, Mods, PointerButton};
pub use host::{Host, MemoryHost};
pub use crate::ops::cell_clipboard_text;

use crate::{
    basic::BasicRunner,
    calc::{refs::MAX_COL, CalcEngine},
    model::{Cell, CellValue, Coord, Sheet, VectorBuffer, Workbook},
    ops::{self, Axis},
    storage::{deserialize_workbook, serialize_workbook},
    tui::{MenuAction, WorkspaceSession},
    view::{Bitmap, ContextMenu, ContextMenuAction, HersheyFont, VectorWorkspaceView},
};

const MIN_WIDTH: usize = 400;
const MIN_HEIGHT: usize = 300;
const DEFAULT_FILE: &str = "demo.lotus";
const BASIC_INSTRUCTION_LIMIT: usize = 100_000;
/// How many edits Undo can walk back through.
const UNDO_LIMIT: usize = 100;
/// Pixels added or removed per Ctrl+Shift+Arrow press.
const RESIZE_STEP_COL: f32 = 10.0;
const RESIZE_STEP_ROW: f32 = 4.0;
const MAX_COL_WIDTH: f32 = 1_000.0;
const MAX_ROW_HEIGHT: f32 = 400.0;

/// A copy of a sheet as it was before an edit, for Undo and Redo.
struct Snapshot {
    sheet_index: usize,
    sheet: Sheet,
    cursor: Coord,
}

/// A question asked on the edit line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prompt {
    Find,
    GoTo,
    ColumnWidth,
    RowHeight,
}

/// What the last Copy put on the clipboard, so a Paste of that same text can
/// move relative references in formulas by the distance it travelled.
struct CopiedRange {
    text: String,
    origin: Coord,
}

pub struct App {
    pub workbook: Workbook,
    pub current_sheet: usize,
    pub session: WorkspaceSession,
    pub context_menu: ContextMenu,
    /// Where Save writes. `None` when the file that was opened could not be
    /// parsed, so a bad or newer-format file is never overwritten by accident.
    pub file_path: Option<String>,
    view: VectorWorkspaceView,
    font: HersheyFont,
    bitmap: Bitmap,
    vec_buf: VectorBuffer,
    dirty: bool,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    prompt: Option<Prompt>,
    last_find: Option<String>,
    last_copy: Option<CopiedRange>,
    /// Sizes remembered across Collapse so Expand can restore them.
    collapsed_cols: BTreeMap<(usize, u32), f32>,
    collapsed_rows: BTreeMap<(usize, u32), f32>,
    /// The column or row being resized by a run of Ctrl+Shift+Arrow presses
    /// (one Undo step for the whole run, not one per press).
    resize_chain: Option<(bool, u32)>,
    dragging: bool,
}

impl App {
    /// Open `file_path` (or `demo.lotus` when none is given) through `host`.
    ///
    /// - missing/unreadable file: start with the sample workbook; Save will create the file;
    /// - file present but unparseable: show the sample workbook, say so in the
    ///   status line, and leave Save disabled so the original is not overwritten.
    pub fn new(host: &mut dyn Host, width: usize, height: usize, file_path: Option<String>) -> Self {
        let requested = file_path.unwrap_or_else(|| String::from(DEFAULT_FILE));
        let mut status = None;

        let (workbook, path) = match host.read_file(&requested) {
            None => (Self::sample_workbook(), Some(requested)),
            Some(bytes) if bytes.iter().all(|b| b.is_ascii_whitespace()) => {
                (Self::sample_workbook(), Some(requested)) // an empty file counts as a new one
            }
            Some(bytes) => match parse_workbook(&bytes) {
                Ok(wb) => (wb, Some(requested)),
                Err(e) => {
                    status = Some(format!(
                        "Could not read {}: {}. Showing the sample workbook; Save is off so the file is left alone.",
                        requested, e
                    ));
                    (Self::sample_workbook(), None)
                }
            },
        };

        let mut session = WorkspaceSession::new();
        session.cursor = Coord::new(0, 0);
        if let Some(msg) = status {
            session.status_message = msg;
        }

        let bitmap = Bitmap::new(width.max(MIN_WIDTH), height.max(MIN_HEIGHT));
        let mut context_menu = ContextMenu::default_context_menu();
        context_menu.set_viewport(bitmap.width as f32, bitmap.height as f32);

        Self {
            workbook,
            current_sheet: 0,
            session,
            context_menu,
            file_path: path,
            view: VectorWorkspaceView::new(),
            font: HersheyFont::load_simplex(),
            bitmap,
            vec_buf: VectorBuffer::with_capacity(1024),
            dirty: true,
            undo: Vec::new(),
            redo: Vec::new(),
            prompt: None,
            last_find: None,
            last_copy: None,
            collapsed_cols: BTreeMap::new(),
            collapsed_rows: BTreeMap::new(),
            resize_chain: None,
            dragging: false,
        }
    }

    pub fn sample_workbook() -> Workbook {
        let mut wb = Workbook::new("demo-01", "Lotus 5-7-11 Demonstration");
        let mut sheet = Sheet::new("main", "Main Model");

        sheet.set_cell(Cell::number(Coord::new(0, 0), 8000.0).with_name("revenue"));
        sheet.set_cell(Cell::number(Coord::new(0, 1), 0.20).with_name("tax_rate"));
        sheet.set_cell(Cell::formula(Coord::new(0, 2), "=revenue * tax_rate").with_name("tax_amount"));

        sheet.set_cell(Cell::text(Coord::new(1, 0), "Projection Summary"));
        sheet.set_cell(Cell::text(Coord::new(1, 1), "Active"));

        wb.sheets.push(sheet);
        wb
    }

    // ---- shell-facing surface -------------------------------------------------

    /// True (once) if anything changed since the last call, so the shell knows to repaint.
    pub fn take_dirty(&mut self) -> bool {
        core::mem::replace(&mut self.dirty, false)
    }

    /// Draw the current state and return the finished frame.
    pub fn render(&mut self) -> &Bitmap {
        let idx = self.active_index();
        self.view.render(
            &self.workbook.sheets[idx],
            &self.session,
            &self.context_menu,
            &mut self.bitmap,
            &mut self.vec_buf,
            &self.font,
        );
        &self.bitmap
    }

    /// The last rendered frame.
    pub fn bitmap(&self) -> &Bitmap {
        &self.bitmap
    }

    pub fn status(&self) -> &str {
        &self.session.status_message
    }

    pub fn handle_event(&mut self, host: &mut dyn Host, event: Event) {
        match event {
            Event::Resize { width, height } => self.resize(width, height),
            Event::PointerMove { x, y, left_down } => self.pointer_move(x, y, left_down),
            Event::PointerDown { x, y, button: PointerButton::Right } => self.right_click(x, y),
            Event::PointerDown { x, y, button: PointerButton::Left } => self.left_click(host, x, y),
            Event::PointerUp { button: PointerButton::Left, .. } => self.end_drag(),
            Event::PointerUp { .. } => {}
            Event::Scroll { cols, rows } => {
                if !self.context_menu.is_visible {
                    self.view.pan(cols, rows);
                    self.dirty = true;
                }
            }
            Event::Key { key, mods } => self.key(host, key, mods),
            Event::Text(ch) => self.text(host, ch),
        }
    }

    // ---- pointer ----------------------------------------------------------------

    fn resize(&mut self, width: usize, height: usize) {
        if width > 0 && height > 0 && (self.bitmap.width != width || self.bitmap.height != height) {
            self.bitmap = Bitmap::new(width, height);
            self.context_menu.set_viewport(width as f32, height as f32);
            self.dirty = true;
        }
    }

    fn cell_at(&mut self, x: f32, y: f32) -> Option<Coord> {
        let idx = self.active_index();
        self.view.hit_test_cell(&self.workbook.sheets[idx], x, y)
    }

    fn pointer_move(&mut self, x: f32, y: f32, left_down: bool) {
        if self.context_menu.is_visible {
            self.context_menu.handle_mouse_move(x, y);
            self.dirty = true;
            return;
        }
        if !self.dragging {
            return;
        }
        if !left_down {
            self.end_drag(); // the button was released outside the window
            return;
        }
        if let Some(coord) = self.cell_at(x, y) {
            if coord != self.session.cursor {
                self.session.cursor = coord;
                self.dirty = true;
            }
        }
    }

    fn end_drag(&mut self) {
        if self.dragging {
            self.dragging = false;
            if self.session.selection().is_none() && !self.session.extend_mode {
                self.session.selection_anchor = None;
            }
        }
    }

    fn right_click(&mut self, x: f32, y: f32) {
        if !self.context_menu.is_visible {
            // Right-clicking outside the selection selects the cell under the
            // pointer, so the menu's Cut/Copy act on what was clicked.
            if let Some(coord) = self.cell_at(x, y) {
                if !self.session.selected_range().contains(coord) {
                    self.session.clear_selection();
                    self.session.cursor = coord;
                }
            }
        }
        self.context_menu.open_at(x, y);
        self.dirty = true;
    }

    fn left_click(&mut self, host: &mut dyn Host, x: f32, y: f32) {
        self.dirty = true;

        if self.context_menu.is_visible {
            if let Some(action) = self.context_menu.handle_mouse_click(x, y) {
                self.run_context_action(host, action);
            }
            return;
        }

        if let Some(coord) = self.cell_at(x, y) {
            if self.prompt.is_some() {
                self.cancel_prompt();
            }
            self.commit_edit();
            self.session.cursor = coord;
            self.session.selection_anchor = Some(coord); // grows if the pointer is dragged
            self.session.extend_mode = false;
            self.dragging = true;
            self.resize_chain = None;
            self.session.status_message = format!("Selected {}", coord.to_a1());
        }
    }

    fn run_context_action(&mut self, host: &mut dyn Host, action: ContextMenuAction) {
        match action {
            ContextMenuAction::Copy => self.copy_selection(host),
            ContextMenuAction::Paste => self.paste(host),
            ContextMenuAction::Cut => self.cut_selection(host),
            ContextMenuAction::Clear => self.clear_selection_cells(),
            ContextMenuAction::Lotus(menu_action) => self.execute(host, menu_action),
            ContextMenuAction::None => {}
        }
    }

    // ---- keyboard -----------------------------------------------------------------

    fn key(&mut self, host: &mut dyn Host, key: Key, mods: Mods) {
        self.dirty = true;

        if self.session.menu.is_active {
            self.menu_key(host, key);
            return;
        }
        if key == Key::Escape && self.context_menu.is_visible {
            self.context_menu.close();
            return;
        }
        if self.session.edit_buffer.is_some() {
            self.edit_key(host, key, mods);
        } else {
            self.grid_key(host, key, mods);
        }
    }

    fn menu_key(&mut self, host: &mut dyn Host, key: Key) {
        match key {
            Key::Left => {
                self.session.menu.handle_key(None, true, false, false, false);
            }
            Key::Right => {
                self.session.menu.handle_key(None, false, true, false, false);
            }
            Key::Enter => {
                let action = self.session.menu.handle_key(None, false, false, true, false);
                self.execute(host, action);
            }
            Key::Escape => {
                self.session.menu.handle_key(None, false, false, false, true);
            }
            _ => {}
        }
    }

    /// Keys while the edit line is open: a cell being edited, or a prompt.
    fn edit_key(&mut self, host: &mut dyn Host, key: Key, mods: Mods) {
        let prompting = self.prompt.is_some();
        match key {
            Key::Left if mods.ctrl => self.session.move_edit_word(-1, mods.shift),
            Key::Right if mods.ctrl => self.session.move_edit_word(1, mods.shift),
            Key::Left => self.session.move_edit_cursor(-1, mods.shift),
            Key::Right => self.session.move_edit_cursor(1, mods.shift),
            Key::Home => self.session.edit_home(mods.shift),
            Key::End => self.session.edit_end(mods.shift),
            Key::Backspace => self.session.backspace(),
            Key::Delete => self.session.delete_forward(),
            Key::Enter => {
                if prompting {
                    self.submit_prompt();
                } else {
                    self.commit_edit();
                }
            }
            Key::Escape => {
                if prompting {
                    self.cancel_prompt();
                } else {
                    self.session.cancel_editing();
                }
            }
            Key::Up | Key::Down | Key::Tab if !prompting => {
                self.commit_edit();
                match key {
                    Key::Up => self.step_cursor(0, -1, false),
                    Key::Down => self.step_cursor(0, 1, false),
                    _ if mods.shift => self.step_cursor(-1, 0, false),
                    _ => self.step_cursor(1, 0, false),
                }
            }
            Key::Char(c) if mods.ctrl => match c.to_ascii_lowercase() {
                'a' => self.session.edit_select_all(),
                'c' => self.copy_edit_text(host),
                'x' => {
                    self.copy_edit_text(host);
                    self.session.delete_selection();
                }
                'v' => {
                    if let Some(text) = host.clipboard_get() {
                        // A single line: line breaks would not survive in a cell.
                        let line: String = text.lines().next().unwrap_or("").into();
                        self.session.insert_str(&line);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn copy_edit_text(&mut self, host: &mut dyn Host) {
        let text = self
            .session
            .selected_text()
            .or_else(|| self.session.edit_buffer.clone())
            .unwrap_or_default();
        if !text.is_empty() {
            host.clipboard_set(&text);
        }
    }

    /// Keys on the grid itself.
    fn grid_key(&mut self, host: &mut dyn Host, key: Key, mods: Mods) {
        let extend = self.session.extend_mode;
        match key {
            Key::Left | Key::Right | Key::Up | Key::Down => {
                let (dc, dr) = match key {
                    Key::Left => (-1, 0),
                    Key::Right => (1, 0),
                    Key::Up => (0, -1),
                    _ => (0, 1),
                };
                if mods.ctrl && mods.shift {
                    self.resize_cursor_line(dc, dr);
                } else if mods.shift {
                    self.view.pan(dc, dr);
                } else if mods.ctrl {
                    let idx = self.active_index();
                    let target = ops::jump_edge(&self.workbook.sheets[idx], self.session.cursor, dc, dr);
                    self.move_cursor_to(target, extend);
                } else {
                    self.step_cursor(dc, dr, extend);
                }
            }
            Key::Home => {
                let c = self.session.cursor;
                let target = if mods.ctrl { Coord::new(0, 0) } else { Coord::new(0, c.row) };
                self.move_cursor_to(target, extend);
            }
            Key::End => {
                let idx = self.active_index();
                let sheet = &self.workbook.sheets[idx];
                let c = self.session.cursor;
                let target = if mods.ctrl {
                    ops::used_range(sheet).map_or(c, |r| r.br)
                } else {
                    let last = sheet.cells.keys().filter(|k| k.row == c.row).map(|k| k.col).max();
                    Coord::new(last.unwrap_or(c.col), c.row)
                };
                self.move_cursor_to(target, extend);
            }
            Key::PageUp | Key::PageDown => {
                let idx = self.active_index();
                let page = self.view.rows_per_page(&self.workbook.sheets[idx], self.bitmap.height as f32) as i32;
                let rows = if key == Key::PageUp { -page } else { page };
                self.view.pan(0, rows);
                self.step_cursor(0, rows, extend);
            }
            Key::Tab => {
                let dc = if mods.shift { -1 } else { 1 };
                self.step_cursor(dc, 0, extend);
            }
            Key::Enter => self.start_editing(),
            Key::Escape => {
                self.session.clear_selection();
                self.dragging = false;
            }
            Key::Delete => self.clear_selection_cells(),
            Key::Backspace => {}
            Key::F(2) => self.start_editing(),
            Key::F(3) => self.find_next_or_ask(),
            Key::F(5) => self.open_prompt(Prompt::GoTo),
            Key::F(8) => self.toggle_extend_mode(),
            Key::F(_) => {}
            Key::Char(c) if mods.ctrl => self.chord(host, c.to_ascii_lowercase(), mods.shift),
            Key::Char(_) => {}
        }
    }

    fn chord(&mut self, host: &mut dyn Host, c: char, shift: bool) {
        match c {
            'c' => self.copy_selection(host),
            'x' => self.cut_selection(host),
            'v' => self.paste(host),
            'z' => self.undo(),
            'y' => self.redo(),
            'a' => self.select_all(),
            'd' => self.execute(host, MenuAction::FillDown),
            'r' => self.execute(host, MenuAction::FillRight),
            'f' => self.open_prompt(Prompt::Find),
            'g' => self.open_prompt(Prompt::GoTo),
            's' => self.execute(host, MenuAction::FileSave),
            '0' if shift => self.execute(host, MenuAction::ColumnExpand),
            '0' => self.execute(host, MenuAction::ColumnCollapse),
            '9' if shift => self.execute(host, MenuAction::RowExpand),
            '9' => self.execute(host, MenuAction::RowCollapse),
            _ => {}
        }
    }

    fn text(&mut self, host: &mut dyn Host, ch: char) {
        if ch.is_control() {
            return;
        }
        self.dirty = true;

        if self.session.menu.is_active {
            let action = self.session.menu.handle_key(Some(ch), false, false, false, false);
            self.execute(host, action);
        } else if ch == '/' && self.session.edit_buffer.is_none() {
            self.session.menu.open();
        } else {
            if self.session.edit_buffer.is_none() {
                self.session.begin_blank_edit(); // typing over a cell replaces it
            }
            self.session.insert_char(ch);
        }
    }

    // ---- cursor and view ----------------------------------------------------------

    /// Index of the active sheet, creating a blank one if the workbook has none.
    fn active_index(&mut self) -> usize {
        if self.workbook.sheets.is_empty() {
            self.workbook.sheets.push(Sheet::new("main", "Main Sheet"));
        }
        if self.current_sheet >= self.workbook.sheets.len() {
            self.current_sheet = 0;
        }
        self.current_sheet
    }

    /// Scroll the view just enough to keep the cursor on screen.
    fn scroll_to_cursor(&mut self) {
        let idx = self.active_index();
        let (w, h) = (self.bitmap.width as f32, self.bitmap.height as f32);
        let cursor = self.session.cursor;
        self.view.ensure_visible(&self.workbook.sheets[idx], cursor, w, h);
    }

    /// Move the cursor to `target`. With `extend` (extend mode), the cell the
    /// cursor left stays as the selection's other corner.
    fn move_cursor_to(&mut self, target: Coord, extend: bool) {
        if extend {
            if self.session.selection_anchor.is_none() {
                self.session.selection_anchor = Some(self.session.cursor);
            }
        } else {
            self.session.clear_selection();
        }
        self.session.cursor = target;
        self.scroll_to_cursor();
    }

    fn step_cursor(&mut self, d_col: i32, d_row: i32, extend: bool) {
        let c = self.session.cursor;
        let col = (i64::from(c.col) + i64::from(d_col)).clamp(0, i64::from(MAX_COL)) as u32;
        let row = (i64::from(c.row) + i64::from(d_row)).clamp(0, 1_048_575) as u32;
        self.move_cursor_to(Coord::new(col, row), extend);
    }

    fn toggle_extend_mode(&mut self) {
        if self.session.extend_mode {
            self.session.extend_mode = false;
            self.session.status_message = String::from("Extend mode off");
        } else {
            self.session.extend_mode = true;
            if self.session.selection_anchor.is_none() {
                self.session.selection_anchor = Some(self.session.cursor);
            }
            self.session.status_message = String::from("Extend mode: arrows grow the selection (F8 or Esc to stop)");
        }
    }

    fn select_all(&mut self) {
        let idx = self.active_index();
        match ops::used_range(&self.workbook.sheets[idx]) {
            Some(r) => {
                self.session.selection_anchor = Some(r.br);
                self.session.cursor = r.tl;
                self.session.status_message = format!("Selected {}", r.to_a1());
            }
            None => self.session.status_message = String::from("Nothing to select"),
        }
    }

    // ---- undo / redo --------------------------------------------------------------

    fn snapshot(&mut self) -> Snapshot {
        let idx = self.active_index();
        Snapshot { sheet_index: idx, sheet: self.workbook.sheets[idx].clone(), cursor: self.session.cursor }
    }

    fn push_undo(&mut self, snap: Snapshot) {
        self.undo.push(snap);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.resize_chain = None;
        self.session.is_dirty = true;
    }

    /// Run an edit on the active sheet as one Undo step. If it fails, the
    /// sheet is put back exactly as it was and nothing is recorded.
    fn edit_sheet<R>(&mut self, edit: impl FnOnce(&mut Sheet) -> Result<R, String>) -> Result<R, String> {
        let snap = self.snapshot();
        let idx = snap.sheet_index;
        match edit(&mut self.workbook.sheets[idx]) {
            Ok(value) => {
                self.push_undo(snap);
                Ok(value)
            }
            Err(e) => {
                self.workbook.sheets[idx] = snap.sheet;
                Err(e)
            }
        }
    }

    fn swap_in(&mut self, snap: Snapshot) -> Snapshot {
        let current = Snapshot {
            sheet_index: snap.sheet_index,
            sheet: core::mem::replace(&mut self.workbook.sheets[snap.sheet_index], snap.sheet),
            cursor: self.session.cursor,
        };
        self.current_sheet = snap.sheet_index;
        self.session.cursor = snap.cursor;
        self.session.clear_selection();
        self.session.is_dirty = true;
        self.resize_chain = None;
        self.scroll_to_cursor();
        current
    }

    fn undo(&mut self) {
        match self.undo.pop() {
            Some(snap) if snap.sheet_index < self.workbook.sheets.len() => {
                let current = self.swap_in(snap);
                self.redo.push(current);
                self.session.status_message = String::from("Undid the last change");
            }
            _ => self.session.status_message = String::from("Nothing to undo"),
        }
    }

    fn redo(&mut self) {
        match self.redo.pop() {
            Some(snap) if snap.sheet_index < self.workbook.sheets.len() => {
                let current = self.swap_in(snap);
                self.undo.push(current);
                self.session.status_message = String::from("Redid the change");
            }
            _ => self.session.status_message = String::from("Nothing to redo"),
        }
    }

    // ---- editing ------------------------------------------------------------------

    fn start_editing(&mut self) {
        let idx = self.active_index();
        self.session.start_editing(&self.workbook.sheets[idx]);
    }

    /// Commit the cell being edited (does nothing during a prompt).
    fn commit_edit(&mut self) {
        if self.prompt.is_some() || self.session.edit_buffer.is_none() {
            return;
        }
        let snap = self.snapshot();
        let idx = snap.sheet_index;
        self.session.commit_editing(&mut self.workbook.sheets[idx]);
        self.push_undo(snap);
    }

    fn clear_selection_cells(&mut self) {
        let range = self.session.selected_range();
        let outcome = self.edit_sheet(|sheet| {
            ops::clear_range(sheet, range);
            Ok(())
        });
        self.session.status_message = match outcome {
            Ok(()) => format!("Cleared {}", range.to_a1()),
            Err(e) => e,
        };
    }

    // ---- clipboard ----------------------------------------------------------------

    fn copy_selection(&mut self, host: &mut dyn Host) {
        let range = self.session.selected_range();
        let idx = self.active_index();
        let text = ops::range_to_text(&self.workbook.sheets[idx], range);
        if text.chars().all(|c| c == '\t' || c == '\n') {
            return; // nothing in the range to copy
        }
        host.clipboard_set(&text);
        self.last_copy = Some(CopiedRange { text, origin: range.tl });
        self.session.status_message = format!("Copied {}", range.to_a1());
    }

    fn cut_selection(&mut self, host: &mut dyn Host) {
        self.copy_selection(host);
        self.clear_selection_cells();
    }

    fn paste(&mut self, host: &mut dyn Host) {
        let Some(text) = host.clipboard_get() else { return };
        let origin = self.session.selection().map_or(self.session.cursor, |r| r.tl);
        // Text we copied ourselves keeps its formulas' relative references.
        let shift = self.last_copy.as_ref().filter(|c| c.text == text).map(|c| {
            (i64::from(origin.col) - i64::from(c.origin.col), i64::from(origin.row) - i64::from(c.origin.row))
        });
        match self.edit_sheet(|sheet| ops::paste_text(sheet, origin, &text, shift)) {
            Ok(pasted) => {
                if pasted.is_single() {
                    self.session.clear_selection();
                } else {
                    self.session.selection_anchor = Some(pasted.br);
                    self.session.cursor = pasted.tl;
                }
                self.session.status_message = format!("Pasted {}", pasted.to_a1());
            }
            Err(e) => self.session.status_message = e,
        }
    }

    // ---- prompts ------------------------------------------------------------------

    fn open_prompt(&mut self, prompt: Prompt) {
        if self.session.edit_buffer.is_some() {
            self.commit_edit();
        }
        let idx = self.active_index();
        let cursor = self.session.cursor;
        let (label, prefill) = match prompt {
            Prompt::Find => ("Find: ", self.last_find.clone()),
            Prompt::GoTo => ("Go to: ", None),
            Prompt::ColumnWidth => {
                ("Column width: ", Some(format!("{}", self.workbook.sheets[idx].col_width(cursor.col))))
            }
            Prompt::RowHeight => {
                ("Row height: ", Some(format!("{}", self.workbook.sheets[idx].row_height(cursor.row))))
            }
        };
        self.session.begin_prompt(label);
        if let Some(text) = prefill {
            self.session.insert_str(&text);
            self.session.edit_select_all(); // typing replaces the suggestion
        }
        self.prompt = Some(prompt);
    }

    fn cancel_prompt(&mut self) {
        self.prompt = None;
        self.session.cancel_editing();
    }

    fn submit_prompt(&mut self) {
        let answer = self.session.end_prompt().unwrap_or_default();
        let answer = answer.trim();
        let Some(prompt) = self.prompt.take() else { return };
        self.session.status_message = String::from("READY");
        match prompt {
            Prompt::Find => self.find(answer),
            Prompt::GoTo => self.go_to(answer),
            Prompt::ColumnWidth => self.set_size(true, answer),
            Prompt::RowHeight => self.set_size(false, answer),
        }
    }

    fn find_next_or_ask(&mut self) {
        match self.last_find.clone() {
            Some(needle) => self.find(&needle),
            None => self.open_prompt(Prompt::Find),
        }
    }

    fn find(&mut self, needle: &str) {
        if needle.is_empty() {
            return;
        }
        self.last_find = Some(String::from(needle));
        let idx = self.active_index();
        match ops::find_next(&self.workbook.sheets[idx], self.session.cursor, needle) {
            Some(hit) => {
                self.move_cursor_to(hit, false);
                self.session.status_message = format!("Found \"{}\" at {}", needle, hit.to_a1());
            }
            None => self.session.status_message = format!("\"{}\" not found", needle),
        }
    }

    fn go_to(&mut self, address: &str) {
        let idx = self.active_index();
        let target = Coord::parse_a1(address).or_else(|| {
            self.workbook.sheets[idx]
                .cells
                .values()
                .find(|c| c.name.as_deref().map_or(false, |n| n.eq_ignore_ascii_case(address)))
                .map(|c| c.coord)
        });
        match target {
            Some(coord) => {
                self.move_cursor_to(coord, false);
                self.session.status_message = format!("Went to {}", coord.to_a1());
            }
            None => self.session.status_message = format!("No cell or name \"{}\"", address),
        }
    }

    // ---- column and row sizes -----------------------------------------------------

    fn set_size(&mut self, is_col: bool, answer: &str) {
        let Ok(size) = answer.parse::<f32>() else {
            self.session.status_message = format!("\"{}\" is not a number", answer);
            return;
        };
        if !size.is_finite() || size <= 0.0 {
            self.session.status_message = format!("\"{}\" is not a usable size", answer);
            return;
        }
        let range = self.session.selected_range();
        let _ = self.edit_sheet(|sheet| {
            if is_col {
                for col in range.tl.col..=range.br.col {
                    sheet.set_col_width(col, size.min(MAX_COL_WIDTH));
                }
            } else {
                for row in range.tl.row..=range.br.row {
                    sheet.set_row_height(row, size.min(MAX_ROW_HEIGHT));
                }
            }
            Ok(())
        });
        self.session.status_message = if is_col { format!("Column width {}", size) } else { format!("Row height {}", size) };
        self.scroll_to_cursor();
    }

    /// Ctrl+Shift+Arrow: grow or shrink the cursor's column (left/right) or
    /// row (up/down). A held key makes one Undo step, not one per repeat.
    fn resize_cursor_line(&mut self, d_col: i32, d_row: i32) {
        let cursor = self.session.cursor;
        let is_col = d_col != 0;
        let line = if is_col { cursor.col } else { cursor.row };
        let chain = (is_col, line);

        if self.resize_chain != Some(chain) {
            let snap = self.snapshot();
            self.push_undo(snap);
        }
        self.resize_chain = Some(chain);

        let idx = self.active_index();
        let sheet = &mut self.workbook.sheets[idx];
        let status = if is_col {
            let width = (sheet.col_width(line) + d_col as f32 * RESIZE_STEP_COL).min(MAX_COL_WIDTH);
            sheet.set_col_width(line, width);
            format!("Column {} width {}", Coord::new(line, 0).to_a1().trim_end_matches(|c: char| c.is_ascii_digit()), sheet.col_width(line))
        } else {
            let height = (sheet.row_height(line) + d_row as f32 * RESIZE_STEP_ROW).min(MAX_ROW_HEIGHT);
            sheet.set_row_height(line, height);
            format!("Row {} height {}", line + 1, sheet.row_height(line))
        };
        self.session.status_message = status;
        self.scroll_to_cursor();
    }

    /// Collapse the selected columns (or rows) to slivers, remembering their sizes.
    fn collapse(&mut self, is_col: bool) {
        let range = self.session.selected_range();
        let idx = self.active_index();
        let snap = self.snapshot();
        let mut changed = 0;
        {
            let sheet = &mut self.workbook.sheets[idx];
            if is_col {
                for col in range.tl.col..=range.br.col {
                    if !ops::is_col_collapsed(sheet, col) {
                        self.collapsed_cols.insert((idx, col), sheet.col_width(col));
                        sheet.set_col_width(col, ops::COLLAPSED_COL_WIDTH);
                        changed += 1;
                    }
                }
            } else {
                for row in range.tl.row..=range.br.row {
                    if !ops::is_row_collapsed(sheet, row) {
                        self.collapsed_rows.insert((idx, row), sheet.row_height(row));
                        sheet.set_row_height(row, ops::COLLAPSED_ROW_HEIGHT);
                        changed += 1;
                    }
                }
            }
        }
        let what = if is_col { "column" } else { "row" };
        if changed > 0 {
            self.push_undo(snap);
            self.session.status_message = format!("Collapsed {} {}(s)", changed, what);
        } else {
            self.session.status_message = String::from("Already collapsed");
        }
    }

    /// Expand the selected collapsed columns (or rows) to their old size,
    /// or the default if the old size is not remembered.
    fn expand(&mut self, is_col: bool) {
        let range = self.session.selected_range();
        let idx = self.active_index();
        let snap = self.snapshot();
        let mut changed = 0;
        {
            let sheet = &mut self.workbook.sheets[idx];
            if is_col {
                for col in range.tl.col..=range.br.col {
                    if ops::is_col_collapsed(sheet, col) {
                        let width = self.collapsed_cols.remove(&(idx, col)).unwrap_or(sheet.default_col_width);
                        sheet.set_col_width(col, width);
                        changed += 1;
                    }
                }
            } else {
                for row in range.tl.row..=range.br.row {
                    if ops::is_row_collapsed(sheet, row) {
                        let height = self.collapsed_rows.remove(&(idx, row)).unwrap_or(sheet.default_row_height);
                        sheet.set_row_height(row, height);
                        changed += 1;
                    }
                }
            }
        }
        let what = if is_col { "column" } else { "row" };
        if changed > 0 {
            self.push_undo(snap);
            self.session.status_message = format!("Expanded {} {}(s)", changed, what);
        } else {
            self.session.status_message = String::from("Not collapsed");
        }
    }

    fn fit_columns(&mut self) {
        let range = self.session.selected_range();
        let scale = self.view.font_scale;
        let idx = self.active_index();
        let snap = self.snapshot();
        {
            let font = &self.font;
            let sheet = &mut self.workbook.sheets[idx];
            for col in range.tl.col..=range.br.col {
                let width = ops::fit_col_width(sheet, col, &|s: &str| font.measure_string(s, scale).0);
                sheet.set_col_width(col, width);
            }
        }
        self.push_undo(snap);
        self.session.status_message = format!("Fitted {}", if range.cols() == 1 { "the column" } else { "the columns" });
    }

    // ---- structural edits ---------------------------------------------------------

    /// Number of document links and formulas that no longer find their target.
    fn broken_link_count(&self) -> usize {
        let report = self.workbook.check_orphans();
        report.broken_document_cell_refs.len() + report.dangling_formula_refs.len()
    }

    fn structural(&mut self, axis: Axis, insert: bool) {
        let range = self.session.selected_range();
        let (at, count) = match axis {
            Axis::Row => (range.tl.row, range.rows()),
            Axis::Col => (range.tl.col, range.cols()),
        };
        let broken_before = self.broken_link_count();
        let outcome = self.edit_sheet(|sheet| {
            if insert { ops::insert_lines(sheet, axis, at, count) } else { ops::delete_lines(sheet, axis, at, count) }
        });
        let noun = match axis {
            Axis::Row => "row",
            Axis::Col => "column",
        };
        self.session.status_message = match outcome {
            Ok(()) => {
                self.session.clear_selection();
                let mut msg = format!("{} {} {}(s)", if insert { "Inserted" } else { "Deleted" }, count, noun);
                let broken_after = self.broken_link_count();
                if broken_after > broken_before {
                    msg.push_str(&format!(
                        "; {} link(s) now broken (Worksheet > Orphans lists them)",
                        broken_after - broken_before
                    ));
                }
                msg
            }
            Err(e) => e,
        };
    }

    fn fill(&mut self, down: bool) {
        let range = self.session.selected_range();
        let outcome = self.edit_sheet(|sheet| if down { ops::fill_down(sheet, range) } else { ops::fill_right(sheet, range) });
        self.session.status_message = match outcome {
            Ok(()) => format!("Filled {}", if down { "down" } else { "right" }),
            Err(e) => e,
        };
    }

    fn sort(&mut self, ascending: bool) {
        let Some(range) = self.session.selection() else {
            self.session.status_message = String::from("Select the rows to sort first (drag, or F8 and arrows)");
            return;
        };
        let key_col = if (range.tl.col..=range.br.col).contains(&self.session.cursor.col) {
            self.session.cursor.col
        } else {
            range.tl.col
        };
        let outcome = self.edit_sheet(|sheet| ops::sort_rows(sheet, range, key_col, ascending));
        self.session.status_message = match outcome {
            Ok(()) => format!(
                "Sorted {} by column {} {}",
                range.to_a1(),
                Coord::new(key_col, 0).to_a1().trim_end_matches(|c: char| c.is_ascii_digit()),
                if ascending { "ascending" } else { "descending" }
            ),
            Err(e) => e,
        };
    }

    // ---- menu commands --------------------------------------------------------------

    /// Run one command from the `/` menu or the context menu.
    pub fn execute(&mut self, host: &mut dyn Host, action: MenuAction) {
        self.dirty = true;
        match action {
            MenuAction::None => {}

            // Edit
            MenuAction::EditUndo => self.undo(),
            MenuAction::EditRedo => self.redo(),
            MenuAction::EditCut => self.cut_selection(host),
            MenuAction::EditCopy => self.copy_selection(host),
            MenuAction::EditPaste => self.paste(host),
            MenuAction::EditClear => self.clear_selection_cells(),
            MenuAction::EditSelectAll => self.select_all(),
            MenuAction::EditFind => self.open_prompt(Prompt::Find),
            MenuAction::EditFindNext => self.find_next_or_ask(),
            MenuAction::EditGoTo => self.open_prompt(Prompt::GoTo),

            // Columns and rows
            MenuAction::InsertColumn => self.structural(Axis::Col, true),
            MenuAction::DeleteColumn => self.structural(Axis::Col, false),
            MenuAction::InsertRow => self.structural(Axis::Row, true),
            MenuAction::DeleteRow => self.structural(Axis::Row, false),
            MenuAction::ColumnWidth => self.open_prompt(Prompt::ColumnWidth),
            MenuAction::RowHeight => self.open_prompt(Prompt::RowHeight),
            MenuAction::ColumnFit => self.fit_columns(),
            MenuAction::ColumnCollapse => self.collapse(true),
            MenuAction::ColumnExpand => self.expand(true),
            MenuAction::RowCollapse => self.collapse(false),
            MenuAction::RowExpand => self.expand(false),

            // Data
            MenuAction::FillDown => self.fill(true),
            MenuAction::FillRight => self.fill(false),
            MenuAction::SortAscending => self.sort(true),
            MenuAction::SortDescending => self.sort(false),

            // Workbook
            MenuAction::FileSave => self.file_save(host),
            MenuAction::FileReload => self.file_reload(host),
            MenuAction::FileInspect => {
                let cells: usize = self.workbook.sheets.iter().map(|s| s.cells.len()).sum();
                self.session.status_message = format!(
                    "{}: {} sheets, {} cells, {} documents",
                    self.workbook.title,
                    self.workbook.sheets.len(),
                    cells,
                    self.workbook.documents.len()
                );
            }
            MenuAction::RecalculateAll => {
                self.session.status_message = match CalcEngine::recalculate_workbook(&mut self.workbook) {
                    Ok(n) => format!("Recalculated {} formulas across {} sheets", n, self.workbook.sheets.len()),
                    Err(e) => format!("Recalculate failed: {}", e),
                };
            }
            MenuAction::OrphanCheck => {
                let report = self.workbook.check_orphans();
                self.session.status_message = format!(
                    "Audit: {} broken refs, {} dangling formulas",
                    report.broken_document_refs.len() + report.broken_document_cell_refs.len(),
                    report.dangling_formula_refs.len()
                );
            }
            MenuAction::BasicRun => self.basic_run(),
            MenuAction::TOCJump => {
                let toc = self.workbook.compute_toc();
                self.move_cursor_to(toc.pos, false);
                self.session.status_message = format!("Jumped to TOC at {}", toc.pos.to_a1());
            }
            MenuAction::Quit => host.request_quit(),
        }
    }

    fn file_save(&mut self, host: &mut dyn Host) {
        let Some(path) = self.file_path.clone() else {
            self.session.status_message =
                String::from("Not saved: no file path (the file that was opened could not be read)");
            return;
        };
        let _ = CalcEngine::recalculate_workbook(&mut self.workbook);
        let bytes = serialize_workbook(&self.workbook);
        self.session.status_message = match host.write_file(&path, &bytes) {
            Ok(()) => {
                self.session.is_dirty = false;
                format!("Saved workbook to {}", path)
            }
            Err(e) => format!("Save failed for {}: {}", path, e),
        };
    }

    fn file_reload(&mut self, host: &mut dyn Host) {
        let Some(path) = self.file_path.clone() else {
            self.session.status_message = String::from("Nothing to reload: no file path");
            return;
        };
        self.session.status_message = match host.read_file(&path) {
            None => format!("Reload failed: could not read {}", path),
            Some(bytes) => match parse_workbook(&bytes) {
                Ok(wb) => {
                    self.workbook = wb;
                    self.undo.clear();
                    self.redo.clear();
                    self.collapsed_cols.clear();
                    self.collapsed_rows.clear();
                    self.session.clear_selection();
                    self.session.is_dirty = false;
                    format!("Reloaded {}", path)
                }
                Err(e) => format!("Reload failed: {}", e),
            },
        };
    }

    fn basic_run(&mut self) {
        let cursor = self.session.cursor;
        let idx = self.active_index();
        let sheet = &mut self.workbook.sheets[idx];

        let source = match sheet.get_cell(cursor).map(|c| &c.value) {
            Some(CellValue::Basic { source }) => Some(source.clone()),
            _ => None,
        };
        self.session.status_message = match source {
            None => String::from("Not a BASIC script cell."),
            Some(src) => match BasicRunner::execute(sheet, cursor, &src, &[], &[], Some(BASIC_INSTRUCTION_LIMIT)) {
                Ok(writes) => {
                    self.session.is_dirty = true;
                    format!("BASIC: executed ({} writes)", writes)
                }
                Err(e) => format!("BASIC Error: {:?}", e),
            },
        };
    }
}

/// Parse a workbook file, refusing anything that is not one.
///
/// The storage reader is lenient: it turns empty input, plain text, HTML or
/// JSON into an empty workbook rather than an error. Opening the wrong file
/// and pressing Save would then silently replace it, so insist on the root tag first.
fn parse_workbook(bytes: &[u8]) -> Result<Workbook, String> {
    const ROOT: &[u8] = b"<workbook";
    if !bytes.windows(ROOT.len()).any(|w| w == ROOT) {
        return Err(String::from("not a Lotus workbook (no <workbook> tag)"));
    }
    deserialize_workbook(bytes).map_err(|e| format!("{:?}", e))
}

#[cfg(test)]
mod tests;
