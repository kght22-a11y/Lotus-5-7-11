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

#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
};

pub mod event;
pub mod host;

pub use event::{Event, Key, Mods, PointerButton};
pub use host::{Host, MemoryHost};

use crate::{
    basic::BasicRunner,
    calc::CalcEngine,
    model::{Cell, CellValue, Coord, Sheet, VectorBuffer, Workbook},
    storage::{deserialize_workbook, serialize_workbook},
    tui::{MenuAction, WorkspaceSession},
    view::{Bitmap, ContextMenu, ContextMenuAction, HersheyFont, VectorWorkspaceView},
};

const MIN_WIDTH: usize = 400;
const MIN_HEIGHT: usize = 300;
const DEFAULT_FILE: &str = "demo.lotus";
const BASIC_INSTRUCTION_LIMIT: usize = 100_000;

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
            Event::PointerMove { x, y } => {
                if self.context_menu.is_visible {
                    self.context_menu.handle_mouse_move(x, y);
                    self.dirty = true;
                }
            }
            Event::PointerDown { x, y, button: PointerButton::Right } => {
                self.context_menu.open_at(x, y);
                self.dirty = true;
            }
            Event::PointerDown { x, y, button: PointerButton::Left } => self.left_click(host, x, y),
            Event::Key { key, mods } => self.key(host, key, mods),
            Event::Text(ch) => self.text(host, ch),
        }
    }

    // ---- input handling -------------------------------------------------------

    fn resize(&mut self, width: usize, height: usize) {
        if width > 0 && height > 0 && (self.bitmap.width != width || self.bitmap.height != height) {
            self.bitmap = Bitmap::new(width, height);
            self.context_menu.set_viewport(width as f32, height as f32);
            self.dirty = true;
        }
    }

    fn left_click(&mut self, host: &mut dyn Host, x: f32, y: f32) {
        self.dirty = true;

        if self.context_menu.is_visible {
            if let Some(action) = self.context_menu.handle_mouse_click(x, y) {
                self.run_context_action(host, action);
            }
            return;
        }

        let idx = self.active_index();
        if let Some(coord) = self.view.hit_test_cell(&self.workbook.sheets[idx], x, y) {
            self.commit_editing();
            self.session.cursor = coord;
            self.session.status_message = format!("Selected {}", coord.to_a1());
        }
    }

    fn run_context_action(&mut self, host: &mut dyn Host, action: ContextMenuAction) {
        match action {
            ContextMenuAction::Copy => self.copy_cell(host),
            ContextMenuAction::Paste => self.paste_cell(host),
            ContextMenuAction::Cut => self.cut_cell(host),
            ContextMenuAction::Clear => self.clear_current_cell(),
            ContextMenuAction::Lotus(menu_action) => self.execute(host, menu_action),
            ContextMenuAction::None => {}
        }
    }

    fn key(&mut self, host: &mut dyn Host, key: Key, mods: Mods) {
        self.dirty = true;
        let menu_active = self.session.menu.is_active;

        match key {
            Key::Up if !menu_active => self.session.move_cursor(0, -1),
            Key::Down if !menu_active => self.session.move_cursor(0, 1),
            Key::Up | Key::Down => {}
            Key::Left => {
                if menu_active {
                    self.session.menu.handle_key(None, true, false, false, false);
                } else {
                    self.session.move_cursor(-1, 0);
                }
            }
            Key::Right => {
                if menu_active {
                    self.session.menu.handle_key(None, false, true, false, false);
                } else {
                    self.session.move_cursor(1, 0);
                }
            }
            Key::Enter => {
                if menu_active {
                    let action = self.session.menu.handle_key(None, false, false, true, false);
                    self.execute(host, action);
                } else if self.session.edit_buffer.is_some() {
                    self.commit_editing();
                } else {
                    self.start_editing();
                }
            }
            Key::Escape => {
                if self.context_menu.is_visible {
                    self.context_menu.close();
                } else if menu_active {
                    self.session.menu.handle_key(None, false, false, false, true);
                } else if self.session.edit_buffer.is_some() {
                    self.session.cancel_editing();
                }
            }
            Key::Backspace => {
                if let Some(buf) = self.session.edit_buffer.as_mut() {
                    buf.pop();
                }
            }
            Key::Delete => {
                if self.session.edit_buffer.is_some() {
                    self.session.edit_buffer = Some(String::new());
                } else {
                    self.clear_current_cell();
                }
            }
            Key::Char(c) if mods.ctrl => match c.to_ascii_lowercase() {
                'c' => self.copy_cell(host),
                'v' => self.paste_cell(host),
                'x' => self.cut_cell(host),
                's' => self.execute(host, MenuAction::FileSave),
                _ => {}
            },
            Key::Char(_) => {}
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
        } else if let Some(buf) = self.session.edit_buffer.as_mut() {
            buf.push(ch);
        } else {
            self.start_editing();
            self.session.edit_buffer = Some(ch.to_string());
        }
    }

    // ---- editing --------------------------------------------------------------

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

    fn start_editing(&mut self) {
        let idx = self.active_index();
        self.session.start_editing(&self.workbook.sheets[idx]);
    }

    fn commit_editing(&mut self) {
        if self.session.edit_buffer.is_some() {
            let idx = self.active_index();
            self.session.commit_editing(&mut self.workbook.sheets[idx]);
        }
    }

    fn clear_current_cell(&mut self) {
        let idx = self.active_index();
        let cursor = self.session.cursor;
        let sheet = &mut self.workbook.sheets[idx];
        sheet.cells.remove(&cursor);
        let _ = CalcEngine::recalculate_sheet(sheet, None);
        self.session.is_dirty = true;
        self.session.status_message = format!("Cleared {}", cursor.to_a1());
    }

    fn copy_cell(&mut self, host: &mut dyn Host) {
        let idx = self.active_index();
        let cursor = self.session.cursor;
        let text = self.workbook.sheets[idx]
            .get_cell(cursor)
            .and_then(|cell| cell_clipboard_text(&cell.value));
        if let Some(text) = text {
            host.clipboard_set(&text);
            self.session.status_message = format!("Copied {}", cursor.to_a1());
        }
    }

    fn cut_cell(&mut self, host: &mut dyn Host) {
        self.copy_cell(host);
        self.clear_current_cell();
    }

    /// Paste goes through the same text-to-cell parsing as typing, so a pasted
    /// `=A1*2`, `42`, `true` or plain text lands exactly as if it had been typed.
    fn paste_cell(&mut self, host: &mut dyn Host) {
        let Some(text) = host.clipboard_get() else { return };
        let idx = self.active_index();
        let cursor = self.session.cursor;

        let in_progress_edit = self.session.edit_buffer.take();
        self.session.edit_buffer = Some(text);
        self.session.commit_editing(&mut self.workbook.sheets[idx]);
        self.session.edit_buffer = in_progress_edit;

        self.session.is_dirty = true;
        self.session.status_message = format!("Pasted into {}", cursor.to_a1());
    }

    // ---- menu commands --------------------------------------------------------

    /// Run one command from the `/` menu or the context menu.
    pub fn execute(&mut self, host: &mut dyn Host, action: MenuAction) {
        self.dirty = true;
        match action {
            MenuAction::None => {}
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
                self.session.cursor = toc.pos;
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

/// Text placed on the clipboard for a cell, or `None` when the cell has nothing copyable.
pub fn cell_clipboard_text(value: &CellValue) -> Option<String> {
    let text = match value {
        CellValue::Number(n) => format!("{}", n),
        CellValue::Text(t) => t.clone(),
        CellValue::Formula { expr, .. } => expr.clone(),
        CellValue::Boolean(b) => String::from(if *b { "TRUE" } else { "FALSE" }),
        _ => String::new(),
    };
    if text.is_empty() { None } else { Some(text) }
}

#[cfg(test)]
mod tests;
