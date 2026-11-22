//! Lotus 5-7-11 Sovereign Workspace — Native Vector GUI
//!
//! Pure Rust, zero-external-dependency desktop interface rendering through
//! Lotus Core's subpixel anti-aliased vector engine and Hershey typography.
//! Direct Win32 blitting, mouse cell selection, and context menus with nested Lotus commands.

#![windows_subsystem = "windows"]

mod win32;

use core::ffi::c_void;
use std::fs;
use std::ptr;
use win32::*;

use lotus_core::{
    calc::CalcEngine,
    deserialize_workbook,
    model::{Cell, CellValue, Coord, Sheet, VectorBuffer, Workbook},
    serialize_workbook,
    tui::{MenuAction, WorkspaceSession},
    view::{
        Bitmap, ContextMenu, ContextMenuAction, HersheyFont, VectorWorkspaceView,
    },
};

const GWLP_USERDATA: i32 = -21;

struct GuiState {
    workbook: Workbook,
    current_sheet_idx: usize,
    session: WorkspaceSession,
    context_menu: ContextMenu,
    view: VectorWorkspaceView,
    font: HersheyFont,
    bitmap: Bitmap,
    vec_buf: VectorBuffer,
    file_path: Option<String>,
}

impl GuiState {
    fn new(width: usize, height: usize, file_path: Option<String>) -> Self {
        let font = HersheyFont::load_simplex();
        let view = VectorWorkspaceView::new();
        let bitmap = Bitmap::new(width.max(400), height.max(300));
        let vec_buf = VectorBuffer::with_capacity(1024);
        let context_menu = ContextMenu::default_context_menu();

        // Load existing workbook or generate sample
        let (workbook, path) = if let Some(ref p) = file_path {
            if let Ok(bytes) = fs::read(p) {
                if let Ok(wb) = deserialize_workbook(&bytes) {
                    (wb, Some(p.clone()))
                } else {
                    (create_sample_workbook(), Some(p.clone()))
                }
            } else {
                (create_sample_workbook(), Some(p.clone()))
            }
        } else if let Ok(bytes) = fs::read("demo.lotus") {
            if let Ok(wb) = deserialize_workbook(&bytes) {
                (wb, Some("demo.lotus".into()))
            } else {
                (create_sample_workbook(), Some("demo.lotus".into()))
            }
        } else {
            (create_sample_workbook(), Some("demo.lotus".into()))
        };

        let mut session = WorkspaceSession::new();
        session.cursor = Coord::new(0, 0);

        Self {
            workbook,
            current_sheet_idx: 0,
            session,
            context_menu,
            view,
            font,
            bitmap,
            vec_buf,
            file_path: path,
        }
    }

    fn current_sheet_mut(&mut self) -> &mut Sheet {
        if self.workbook.sheets.is_empty() {
            self.workbook.sheets.push(Sheet::new("main", "Main Sheet"));
        }
        if self.current_sheet_idx >= self.workbook.sheets.len() {
            self.current_sheet_idx = 0;
        }
        &mut self.workbook.sheets[self.current_sheet_idx]
    }

    fn current_sheet(&self) -> &Sheet {
        self.workbook.sheets.get(self.current_sheet_idx).unwrap_or_else(|| {
            panic!("No active sheet available");
        })
    }

    fn render(&mut self) {
        if self.workbook.sheets.is_empty() {
            self.workbook.sheets.push(Sheet::new("main", "Main Sheet"));
        }
        let sheet_idx = self.current_sheet_idx.min(self.workbook.sheets.len() - 1);
        let sheet = &self.workbook.sheets[sheet_idx];
        self.view.render(
            sheet,
            &self.session,
            &self.context_menu,
            &mut self.bitmap,
            &mut self.vec_buf,
            &self.font,
        );
    }

    fn resize(&mut self, width: usize, height: usize) {
        if width > 0 && height > 0 && (self.bitmap.width != width || self.bitmap.height != height) {
            self.bitmap = Bitmap::new(width, height);
            self.render();
        }
    }

    fn start_editing(&mut self) {
        if !self.workbook.sheets.is_empty() {
            let idx = self.current_sheet_idx.min(self.workbook.sheets.len() - 1);
            self.session.start_editing(&self.workbook.sheets[idx]);
        }
    }

    fn commit_editing(&mut self) {
        if self.session.edit_buffer.is_some() && !self.workbook.sheets.is_empty() {
            let idx = self.current_sheet_idx.min(self.workbook.sheets.len() - 1);
            self.session.commit_editing(&mut self.workbook.sheets[idx]);
        }
    }

    fn clear_current_cell(&mut self) {
        if self.workbook.sheets.is_empty() {
            return;
        }
        let idx = self.current_sheet_idx.min(self.workbook.sheets.len() - 1);
        let cursor = self.session.cursor;
        let sheet = &mut self.workbook.sheets[idx];
        sheet.cells.remove(&cursor);
        let _ = CalcEngine::recalculate_sheet(sheet, None);
        self.session.status_message = format!("Cleared {}", cursor.to_a1());
    }

    fn execute_lotus_action(&mut self, action: MenuAction) {
        match action {
            MenuAction::FileSave => {
                let sheet = self.current_sheet_mut();
                let _ = CalcEngine::recalculate_sheet(sheet, None);
                if let Some(ref path) = self.file_path {
                    let bytes = serialize_workbook(&self.workbook);
                    let _ = fs::write(path, bytes);
                    self.session.status_message = format!("Saved workbook to {}", path);
                } else {
                    let bytes = serialize_workbook(&self.workbook);
                    let _ = fs::write("demo.lotus", bytes);
                    self.session.status_message = "Saved workbook to demo.lotus".into();
                }
            }
            MenuAction::FileReload => {
                if let Some(ref path) = self.file_path {
                    if let Ok(bytes) = fs::read(path) {
                        if let Ok(wb) = deserialize_workbook(&bytes) {
                            self.workbook = wb;
                            self.session.status_message = format!("Reloaded {}", path);
                        }
                    }
                }
            }
            MenuAction::RecalculateAll => {
                let sheet = self.current_sheet_mut();
                let _ = CalcEngine::recalculate_sheet(sheet, None);
                self.session.status_message = "All formulas recalculated.".into();
            }
            MenuAction::OrphanCheck => {
                let report = self.workbook.check_orphans();
                self.session.status_message = format!(
                    "Audit: {} broken refs, {} dangling formulas",
                    report.broken_document_refs.len() + report.broken_document_cell_refs.len(),
                    report.dangling_formula_refs.len()
                );
            }
            MenuAction::BasicRun => {
                let cursor = self.session.cursor;
                let sheet = self.current_sheet_mut();
                if let Some(cell) = sheet.get_cell(cursor) {
                    if let CellValue::Basic { ref source } = cell.value {
                        let source_clone = source.clone();
                        let res = lotus_core::BasicRunner::execute(sheet, cursor, &source_clone, &[], &[], Some(100_000));
                        match res {
                            Ok(w) => self.session.status_message = format!("BASIC: executed ({} writes)", w),
                            Err(e) => self.session.status_message = format!("BASIC Error: {:?}", e),
                        }
                    } else {
                        self.session.status_message = "Not a BASIC script cell.".into();
                    }
                }
            }
            MenuAction::TOCJump => {
                let toc = self.workbook.compute_toc();
                self.session.cursor = toc.pos;
                self.session.status_message = format!("Jumped to TOC at {}", toc.pos.to_a1());
            }
            MenuAction::Quit => {
                unsafe { PostQuitMessage(0); }
            }
            MenuAction::FileInspect | MenuAction::None => {}
        }
    }

    fn copy_cell_to_clipboard(&self, hwnd: HWND) {
        let sheet = self.current_sheet();
        if let Some(cell) = sheet.get_cell(self.session.cursor) {
            let text = match &cell.value {
                CellValue::Number(n) => format!("{}", n),
                CellValue::Text(t) => t.clone(),
                CellValue::Formula { expr, .. } => expr.clone(),
                CellValue::Boolean(b) => (if *b { "TRUE" } else { "FALSE" }).into(),
                _ => String::new(),
            };
            if !text.is_empty() {
                Win32Clipboard::set_text(hwnd, &text);
            }
        }
    }

    fn paste_from_clipboard(&mut self, hwnd: HWND) {
        if let Some(text) = Win32Clipboard::get_text(hwnd) {
            let cursor = self.session.cursor;
            let text = text.trim();
            let sheet = self.current_sheet_mut();
            if text.is_empty() {
                sheet.cells.remove(&cursor);
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

                if let Some(cell) = sheet.cells.get_mut(&cursor) {
                    cell.value = cell_val;
                } else {
                    sheet.set_cell(Cell::new(cursor, cell_val));
                }
            }
            let _ = CalcEngine::recalculate_sheet(sheet, None);
            self.session.status_message = format!("Pasted into {}", cursor.to_a1());
        }
    }
}

fn create_sample_workbook() -> Workbook {
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

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GuiState;

    if state_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    let state = &mut *state_ptr;

    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = core::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            state.render();

            let width = state.bitmap.width as i32;
            let height = state.bitmap.height as i32;

            let mut bmi: BITMAPINFO = core::mem::zeroed();
            bmi.bmiHeader.biSize = core::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = width;
            bmi.bmiHeader.biHeight = -height; // Top-down DIB
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB;

            SetDIBitsToDevice(
                hdc,
                0,
                0,
                width as u32,
                height as u32,
                0,
                0,
                0,
                height as u32,
                state.bitmap.pixels.as_ptr() as *const c_void,
                &bmi,
                DIB_RGB_COLORS,
            );

            EndPaint(hwnd, &ps);
            0
        }

        WM_SIZE => {
            let width = (lparam & 0xFFFF) as usize;
            let height = ((lparam >> 16) & 0xFFFF) as usize;
            state.resize(width, height);
            InvalidateRect(hwnd, ptr::null(), 0);
            0
        }

        WM_LBUTTONDOWN => {
            let mx = (lparam & 0xFFFF) as i16 as f32;
            let my = ((lparam >> 16) & 0xFFFF) as i16 as f32;

            // 1. Context Menu Click
            if state.context_menu.is_visible {
                let action = state.context_menu.handle_mouse_click(mx, my);
                if let Some(act) = action {
                    match act {
                        ContextMenuAction::Copy => state.copy_cell_to_clipboard(hwnd),
                        ContextMenuAction::Paste => state.paste_from_clipboard(hwnd),
                        ContextMenuAction::Cut => {
                            state.copy_cell_to_clipboard(hwnd);
                            state.clear_current_cell();
                        }
                        ContextMenuAction::Clear => {
                            state.clear_current_cell();
                        }
                        ContextMenuAction::Lotus(menu_act) => {
                            state.execute_lotus_action(menu_act);
                        }
                        ContextMenuAction::None => {}
                    }
                }
                InvalidateRect(hwnd, ptr::null(), 0);
                return 0;
            }

            // 2. Cell Selection Click
            let sheet = state.current_sheet();
            if let Some(clicked_coord) = state.view.hit_test_cell(sheet, mx, my) {
                state.commit_editing();
                state.session.cursor = clicked_coord;
                state.session.status_message = format!("Selected {}", clicked_coord.to_a1());
            }

            InvalidateRect(hwnd, ptr::null(), 0);
            0
        }

        WM_RBUTTONDOWN => {
            let mx = (lparam & 0xFFFF) as i16 as f32;
            let my = ((lparam >> 16) & 0xFFFF) as i16 as f32;

            state.context_menu.open_at(mx, my);
            InvalidateRect(hwnd, ptr::null(), 0);
            0
        }

        WM_MOUSEMOVE => {
            if state.context_menu.is_visible {
                let mx = (lparam & 0xFFFF) as i16 as f32;
                let my = ((lparam >> 16) & 0xFFFF) as i16 as f32;
                state.context_menu.handle_mouse_move(mx, my);
                InvalidateRect(hwnd, ptr::null(), 0);
            }
            0
        }

        WM_KEYDOWN => {
            let is_ctrl = (GetKeyState(0x11) as i16) < 0; // VK_CONTROL

            match wparam {
                VK_UP => {
                    if !state.session.menu.is_active {
                        state.session.move_cursor(0, -1);
                    }
                }
                VK_DOWN => {
                    if !state.session.menu.is_active {
                        state.session.move_cursor(0, 1);
                    }
                }
                VK_LEFT => {
                    if state.session.menu.is_active {
                        state.session.menu.handle_key(None, true, false, false, false);
                    } else {
                        state.session.move_cursor(-1, 0);
                    }
                }
                VK_RIGHT => {
                    if state.session.menu.is_active {
                        state.session.menu.handle_key(None, false, true, false, false);
                    } else {
                        state.session.move_cursor(1, 0);
                    }
                }
                VK_RETURN => {
                    if state.session.menu.is_active {
                        let action = state.session.menu.handle_key(None, false, false, true, false);
                        state.execute_lotus_action(action);
                    } else if state.session.edit_buffer.is_some() {
                        state.commit_editing();
                    } else {
                        state.start_editing();
                    }
                }
                VK_ESCAPE => {
                    if state.context_menu.is_visible {
                        state.context_menu.close();
                    } else if state.session.menu.is_active {
                        state.session.menu.handle_key(None, false, false, false, true);
                    } else if state.session.edit_buffer.is_some() {
                        state.session.cancel_editing();
                    }
                }
                VK_BACK => {
                    if let Some(ref mut buf) = state.session.edit_buffer {
                        buf.pop();
                    }
                }
                VK_DELETE => {
                    if state.session.edit_buffer.is_some() {
                        state.session.edit_buffer = Some(String::new());
                    } else {
                        state.clear_current_cell();
                    }
                }
                _ => {
                    // Check Ctrl shortcuts
                    if is_ctrl {
                        match wparam {
                            0x43 => state.copy_cell_to_clipboard(hwnd), // Ctrl+C
                            0x56 => state.paste_from_clipboard(hwnd),   // Ctrl+V
                            0x58 => {                                    // Ctrl+X
                                state.copy_cell_to_clipboard(hwnd);
                                let cursor = state.session.cursor;
                                let sheet = state.current_sheet_mut();
                                sheet.cells.remove(&cursor);
                                let _ = CalcEngine::recalculate_sheet(sheet, None);
                            }
                            0x53 => state.execute_lotus_action(MenuAction::FileSave), // Ctrl+S
                            _ => {}
                        }
                    }
                }
            }

            InvalidateRect(hwnd, ptr::null(), 0);
            0
        }

        WM_CHAR => {
            let is_ctrl = (GetKeyState(0x11) as i16) < 0;
            let ch = char::from_u32(wparam as u32).unwrap_or('\0');

            if !is_ctrl && ch >= ' ' {
                if state.session.menu.is_active {
                    let action = state.session.menu.handle_key(Some(ch), false, false, false, false);
                    state.execute_lotus_action(action);
                } else if ch == '/' && state.session.edit_buffer.is_none() {
                    state.session.menu.open();
                } else if let Some(ref mut buf) = state.session.edit_buffer {
                    buf.push(ch);
                } else {
                    state.start_editing();
                    state.session.edit_buffer = Some(ch.to_string());
                }
            }

            InvalidateRect(hwnd, ptr::null(), 0);
            0
        }

        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file_path = args.get(1).cloned();

    let class_name = to_wide_chars("Lotus5711WindowClass");
    let window_title = to_wide_chars("Lotus 5-7-11 Sovereign Workspace");

    unsafe {
        let h_instance = GetModuleHandleW(ptr::null());

        let mut wc: WNDCLASSEXW = core::mem::zeroed();
        wc.cbSize = core::mem::size_of::<WNDCLASSEXW>() as u32;
        wc.style = CS_HREDRAW | CS_VREDRAW | CS_OWNDC;
        wc.lpfnWndProc = Some(window_proc);
        wc.hInstance = h_instance;
        wc.hCursor = LoadCursorW(ptr::null_mut(), 32512 as *const u16); // IDC_ARROW
        wc.lpszClassName = class_name.as_ptr();

        RegisterClassExW(&wc);

        let initial_width = 850;
        let initial_height = 620;

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            100,
            100,
            initial_width,
            initial_height,
            ptr::null_mut(),
            ptr::null_mut(),
            h_instance,
            ptr::null_mut(),
        );

        if hwnd.is_null() {
            eprintln!("Failed to create Win32 window.");
            return;
        }

        let mut client_rect: RECT = core::mem::zeroed();
        GetClientRect(hwnd, &mut client_rect);
        let client_w = (client_rect.right - client_rect.left) as usize;
        let client_h = (client_rect.bottom - client_rect.top) as usize;

        let state = Box::new(GuiState::new(client_w, client_h, file_path));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

        ShowWindow(hwnd, 1);
        UpdateWindow(hwnd);

        let mut msg: MSG = core::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Clean up heap state on window exit
        let state_raw = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut GuiState;
        if !state_raw.is_null() {
            drop(Box::from_raw(state_raw));
        }
    }
}
