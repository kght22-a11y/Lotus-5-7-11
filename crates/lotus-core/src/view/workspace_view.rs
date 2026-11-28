//! Sovereign vector workspace screen renderer and spatial hit-tester.
//!
//! Renders the classic Lotus 1-2-3 two-line control panel, dynamic column and row headers,
//! reactive spreadsheet cells, multi-cell spans, and context menu directly to `Bitmap` and `VectorBuffer`.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec, vec::Vec};

use crate::{
    model::{
        coord::Coord,
        sheet::Sheet,
        vector::{ColorRgba, Rect2D, VectorBuffer},
        CellValue,
    },
    tui::editor::WorkspaceSession,
    view::{gui::ContextMenu, Bitmap, HersheyFont, SubpixelRasterizer, Transform2D},
};

pub struct VectorWorkspaceView {
    pub header_row_height: f32,
    pub header_col_width: f32,
    pub control_panel_height: f32,
    pub font_scale: f32,
    pub scroll_col: u32,
    pub scroll_row: u32,
}

impl Default for VectorWorkspaceView {
    fn default() -> Self {
        Self {
            header_row_height: 24.0,
            header_col_width: 50.0,
            control_panel_height: 48.0, // 2 lines: 24px status + 24px edit/menu
            font_scale: 0.75,
            scroll_col: 0,
            scroll_row: 0,
        }
    }
}

impl VectorWorkspaceView {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pan/move the viewport by delta columns and delta rows.
    pub fn pan(&mut self, delta_col: i32, delta_row: i32) {
        let new_col = (self.scroll_col as i64 + i64::from(delta_col)).clamp(0, 16_383) as u32;
        let new_row = (self.scroll_row as i64 + i64::from(delta_row)).clamp(0, 1_048_575) as u32;
        self.scroll_col = new_col;
        self.scroll_row = new_row;
    }

    /// Scroll the minimum distance needed so `coord` is fully inside a window
    /// of `view_w` x `view_h` pixels. A cell wider than the window is aligned
    /// to its left edge instead.
    pub fn ensure_visible(&mut self, sheet: &Sheet, coord: Coord, view_w: f32, view_h: f32) {
        if coord.col < self.scroll_col {
            self.scroll_col = coord.col;
        } else {
            while self.scroll_col < coord.col && self.cell_rect(sheet, coord).max_x > view_w {
                self.scroll_col += 1;
            }
        }
        if coord.row < self.scroll_row {
            self.scroll_row = coord.row;
        } else {
            while self.scroll_row < coord.row && self.cell_rect(sheet, coord).max_y > view_h {
                self.scroll_row += 1;
            }
        }
    }

    /// How many default-height rows fit in a window `view_h` pixels tall.
    pub fn rows_per_page(&self, sheet: &Sheet, view_h: f32) -> u32 {
        let grid_h = (view_h - self.control_panel_height - self.header_row_height).max(0.0);
        ((grid_h / sheet.default_row_height.max(1.0)) as u32).max(1)
    }

    /// Calculate the top-left (x, y) and (width, height) bounding rectangle of a grid cell
    /// relative to the current viewport scroll position.
    pub fn cell_rect(&self, sheet: &Sheet, coord: Coord) -> Rect2D {
        let mut x = self.header_col_width;
        if coord.col >= self.scroll_col {
            for c in self.scroll_col..coord.col {
                x += sheet.col_width(c) as f32;
            }
        } else {
            for c in coord.col..self.scroll_col {
                x -= sheet.col_width(c) as f32;
            }
        }

        let mut y = self.control_panel_height + self.header_row_height;
        if coord.row >= self.scroll_row {
            for r in self.scroll_row..coord.row {
                y += sheet.row_height(r) as f32;
            }
        } else {
            for r in coord.row..self.scroll_row {
                y -= sheet.row_height(r) as f32;
            }
        }

        let cell = sheet.get_cell(coord);
        let col_span = cell.map(|c| c.col_span).unwrap_or(1);
        let row_span = cell.map(|c| c.row_span).unwrap_or(1);

        let mut w = 0.0;
        for c in coord.col..(coord.col + col_span) {
            w += sheet.col_width(c) as f32;
        }

        let mut h = 0.0;
        for r in coord.row..(coord.row + row_span) {
            h += sheet.row_height(r) as f32;
        }

        Rect2D::new(x, y, x + w, y + h)
    }

    /// Convert a mouse screen position (x, y) to a spreadsheet `Coord`, accounting for viewport scroll.
    pub fn hit_test_cell(&self, sheet: &Sheet, screen_x: f32, screen_y: f32) -> Option<Coord> {
        let grid_min_x = self.header_col_width;
        let grid_min_y = self.control_panel_height + self.header_row_height;

        if screen_x < grid_min_x || screen_y < grid_min_y {
            return None;
        }

        // Determine column starting from scroll_col
        let mut cur_x = grid_min_x;
        let mut target_col = None;
        for c in self.scroll_col..(self.scroll_col + 1000) {
            let w = sheet.col_width(c) as f32;
            if screen_x >= cur_x && screen_x < cur_x + w {
                target_col = Some(c);
                break;
            }
            cur_x += w;
            if cur_x > screen_x + 500.0 {
                break;
            }
        }

        // Determine row starting from scroll_row
        let mut cur_y = grid_min_y;
        let mut target_row = None;
        for r in self.scroll_row..(self.scroll_row + 1000) {
            let h = sheet.row_height(r) as f32;
            if screen_y >= cur_y && screen_y < cur_y + h {
                target_row = Some(r);
                break;
            }
            cur_y += h;
            if cur_y > screen_y + 500.0 {
                break;
            }
        }

        match (target_col, target_row) {
            (Some(c), Some(r)) => Some(Coord::new(c, r)),
            _ => None,
        }
    }

    /// Renders the complete sovereign workspace scene onto the provided bitmap and vector buffer.
    pub fn render(
        &self,
        sheet: &Sheet,
        session: &WorkspaceSession,
        context_menu: &ContextMenu,
        bitmap: &mut Bitmap,
        vec_buf: &mut VectorBuffer,
        font: &HersheyFont,
    ) {
        let bmp_w = bitmap.width as f32;
        let bmp_h = bitmap.height as f32;

        // 1. Workspace Background: Dark Slate (0x000E141E)
        bitmap.clear(0x000E141E);

        // 2. Control Panel (Top 48px)
        // Line 1: Status Line (0x0016202C)
        bitmap.fill_rect(0, 0, bitmap.width, 24, 0x0016202C);
        // Line 2: Edit / Menu Line (0x001E2B3B)
        bitmap.fill_rect(0, 24, bitmap.width, 24, 0x001E2B3B);

        // Divider between control panel and grid
        bitmap.fill_rect(0, 48, bitmap.width, 1, 0x003A4D66);

        // --- Render Control Panel Text ---
        // Line 1: Sheet title, Coord, Name, Value, Status
        let cell_info = match sheet.get_cell(session.cursor) {
            Some(cell) => {
                let name_tag = cell.name.as_deref().map(|n| format!(" ({})", n)).unwrap_or_default();
                let val_str = match &cell.value {
                    CellValue::Formula { expr, cached_num, cached_text, cached_bool } => {
                        let cached = cached_num.map(|n| format!("{:.2}", n))
                            .or_else(|| cached_bool.map(|b| if b { "TRUE".into() } else { "FALSE".into() }))
                            .or_else(|| cached_text.clone())
                            .unwrap_or_else(|| "<pending>".into());
                        format!("{} => {}", expr, cached)
                    }
                    CellValue::Number(n) => format!("{:.2}", n),
                    CellValue::Text(t) => format!("\"{}\"", t),
                    CellValue::Boolean(b) => (if *b { "TRUE" } else { "FALSE" }).into(),
                    CellValue::Basic { .. } => "BASIC Script".into(),
                    CellValue::Blob { bytes } => format!("Blob ({} bytes)", bytes.len()),
                    CellValue::Error(e) => format!("ERR: {}", e),
                    CellValue::Empty => "<empty>".into(),
                };
                format!("{} | {} | {}", session.cursor.to_a1(), name_tag, val_str)
            }
            None => format!("{} | <empty>", session.cursor.to_a1()),
        };

        let mode_str = if session.menu.is_active {
            "[MENU]"
        } else if session.edit_prompt.is_some() {
            "[PROMPT]"
        } else if session.edit_buffer.is_some() {
            "[EDIT]"
        } else if session.extend_mode {
            "[EXTEND]"
        } else if session.selection().is_some() {
            "[SELECT]"
        } else {
            "[READY]"
        };

        vec_buf.set_width(1.0);
        vec_buf.set_color(ColorRgba::rgb(180, 210, 240));
        let l1_text = format!("{} | Sheet: {}  {}", mode_str, sheet.name, cell_info);
        font.draw_string(&l1_text, 10.0, 16.0, 0.70, vec_buf);

        // Line 2: Live Edit or Lotus '/' Menu
        if session.menu.is_active {
            let (menu_l1, _menu_l2) = session.menu.render_lines();
            vec_buf.set_color(ColorRgba::rgb(0, 230, 160)); // Cyan / Lotus Accent
            font.draw_string(&menu_l1, 10.0, 40.0, 0.70, vec_buf);
        } else if let Some(ref edit_text) = session.edit_buffer {
            let prefix = session.edit_prompt.as_deref().unwrap_or("EDIT: ");
            let (pw, _) = font.measure_string(prefix, 0.75);

            // Highlight selected text range in Navy Blue behind text
            if let Some((sel_start, sel_end)) = session.selection_range() {
                let start_idx = sel_start.min(edit_text.len());
                let end_idx = sel_end.min(edit_text.len());
                let before_sel = &edit_text[..start_idx];
                let (before_w, _) = font.measure_string(before_sel, 0.75);
                let sel_str = &edit_text[start_idx..end_idx];
                let (sel_w, _) = font.measure_string(sel_str, 0.75);

                let highlight_x = (10.0 + pw + before_w) as usize;
                let highlight_y = 26;
                bitmap.fill_rect(highlight_x, highlight_y, (sel_w.ceil() as usize).max(2), 20, 0x00000080);
            }

            vec_buf.set_color(ColorRgba::rgb(255, 204, 0)); // Bright Amber for edit line
            font.draw_string(prefix, 10.0, 40.0, 0.75, vec_buf);

            let text_x = 10.0 + pw;
            font.draw_string(edit_text, text_x, 40.0, 0.75, vec_buf);

            // Draw vertical text cursor
            let cur_idx = session.edit_cursor.min(edit_text.len());
            let before_cur = &edit_text[..cur_idx];
            let (cur_offset, _) = font.measure_string(before_cur, 0.75);
            let cur_x = text_x + cur_offset;
            vec_buf.set_color(ColorRgba::WHITE);
            vec_buf.set_width(1.5);
            vec_buf.move_to(cur_x, 26.0);
            vec_buf.line_to(cur_x, 44.0);
        } else {
            vec_buf.set_color(ColorRgba::rgb(140, 160, 180));
            let message = session.status_message.as_str();
            if message.is_empty() || message == "READY" {
                font.draw_string("READY (Press '/' for Lotus Menu, Right-Click for Context Menu)", 10.0, 40.0, 0.65, vec_buf);
            } else {
                font.draw_string(message, 10.0, 40.0, 0.65, vec_buf);
            }
        }

        // 3. Headers & Grid Dimensions
        let grid_y0 = self.control_panel_height + self.header_row_height;

        // Header Backgrounds: Classic Header Tint (0x001B2430)
        bitmap.fill_rect(0, self.control_panel_height as usize, bitmap.width, self.header_row_height as usize, 0x001B2430);
        bitmap.fill_rect(0, self.control_panel_height as usize, self.header_col_width as usize, bitmap.height - self.control_panel_height as usize, 0x001B2430);

        // Top-left corner box
        bitmap.fill_rect(0, self.control_panel_height as usize, self.header_col_width as usize, self.header_row_height as usize, 0x00243040);

        // 4. Draw Column Headers (starting from self.scroll_col)
        let mut cur_col_x = self.header_col_width;
        let mut visible_cols = Vec::new();
        let mut c = self.scroll_col;
        while cur_col_x < bmp_w && c < 16_384 {
            let w = sheet.col_width(c) as f32;
            let col_name = Coord::new(c, 0).to_a1();
            let col_letter = col_name.trim_end_matches(|ch: char| ch.is_ascii_digit());

            // Header separator line
            bitmap.fill_rect((cur_col_x + w) as usize, self.control_panel_height as usize, 1, bitmap.height - self.control_panel_height as usize, 0x00283648);

            // Centered Column Header text
            let (tw, _) = font.measure_string(col_letter, 0.70);
            let tx = cur_col_x + (w - tw) * 0.5;
            let ty = self.control_panel_height + 17.0;
            vec_buf.set_color(ColorRgba::rgb(200, 220, 240));
            font.draw_string(col_letter, tx, ty, 0.70, vec_buf);

            visible_cols.push(c);
            cur_col_x += w;
            c += 1;
        }

        // 5. Draw Row Headers (starting from self.scroll_row)
        let mut cur_row_y = grid_y0;
        let mut visible_rows = Vec::new();
        let mut r = self.scroll_row;
        while cur_row_y < bmp_h && r < 1_048_576 {
            let h = sheet.row_height(r) as f32;
            let row_str = format!("{}", r + 1);

            // Row separator line
            bitmap.fill_rect(0, (cur_row_y + h) as usize, bitmap.width, 1, 0x00283648);

            // Centered Row Header text
            let (rw, _) = font.measure_string(&row_str, 0.70);
            let rx = (self.header_col_width - rw) * 0.5;
            let ry = cur_row_y + (h * 0.5) + 5.0;
            vec_buf.set_color(ColorRgba::rgb(200, 220, 240));
            font.draw_string(&row_str, rx, ry, 0.70, vec_buf);

            visible_rows.push(r);
            cur_row_y += h;
            r += 1;
        }

        // 5b. Highlight the selected range behind the cells.
        if let Some(range) = session.selection() {
            for &r in &visible_rows {
                for &c in &visible_cols {
                    if range.contains(Coord::new(c, r)) {
                        let rect = self.cell_rect(sheet, Coord::new(c, r));
                        let x0 = rect.min_x.max(self.header_col_width);
                        let y0 = rect.min_y.max(grid_y0);
                        let x1 = rect.max_x.min(bmp_w);
                        let y1 = rect.max_y.min(bmp_h);
                        if x1 > x0 && y1 > y0 {
                            bitmap.fill_rect(x0 as usize, y0 as usize, (x1 - x0) as usize, (y1 - y0) as usize, 0x00183050);
                        }
                    }
                }
            }
        }

        // 6. Render Spreadsheet Cells
        for &r in &visible_rows {
            for &c in &visible_cols {
                let coord = Coord::new(c, r);
                if let Some(cell) = sheet.get_cell(coord) {
                    let rect = self.cell_rect(sheet, coord);

                    // Cell content string
                    let display_val = cell.value.display_text();

                    // Text needs room: collapsed rows and columns show no text.
                    if !display_val.is_empty() && rect.height() >= 20.0 && rect.width() >= 16.0 {
                        let text_color = match &cell.value {
                            CellValue::Formula { .. } => ColorRgba::rgb(100, 220, 255), // Cyan for formulas
                            CellValue::Number(_) => ColorRgba::rgb(230, 240, 255),
                            _ => ColorRgba::rgb(210, 220, 230),
                        };
                        let numeric = matches!(&cell.value, CellValue::Number(_))
                            || matches!(&cell.value, CellValue::Formula { cached_num: Some(_), .. });
                        let shown = fit_text(font, &display_val, self.font_scale, rect.width() - 10.0, numeric);

                        vec_buf.set_color(text_color);
                        let text_x = rect.min_x + 6.0;
                        let text_y = rect.min_y + 16.0;
                        font.draw_string(&shown, text_x, text_y, self.font_scale, vec_buf);
                    }

                    // Check for obfuscated data indicator (red notch)
                    if sheet.cell_obfuscates_data(coord) {
                        let notch_x = (rect.max_x - 3.0) as usize;
                        let notch_y = rect.min_y as usize;
                        bitmap.fill_rect(notch_x, notch_y, 3, rect.height() as usize, 0x00FF3333);
                    }
                }
            }
        }

        // 7. Active Cell Selection Cursor Box
        let cursor_rect = self.cell_rect(sheet, session.cursor);
        if cursor_rect.max_x > self.header_col_width && cursor_rect.max_y > grid_y0
            && cursor_rect.min_x < bmp_w && cursor_rect.min_y < bmp_h
        {
            vec_buf.set_width(2.0);
            vec_buf.set_color(ColorRgba::rgb(0, 230, 160)); // Lotus Cyan Focus Box
            vec_buf.rect(cursor_rect.min_x, cursor_rect.min_y, cursor_rect.width(), cursor_rect.height());

            // Inner soft glow line
            vec_buf.set_width(1.0);
            vec_buf.set_color(ColorRgba::WHITE);
            vec_buf.rect(cursor_rect.min_x + 1.0, cursor_rect.min_y + 1.0, cursor_rect.width() - 2.0, cursor_rect.height() - 2.0);
        }

        // 8. Rasterize Vector Paths (strokes, headers, cell text) onto the bitmap
        SubpixelRasterizer::render(bitmap, vec_buf, &Transform2D::identity());
        vec_buf.clear();

        // 9. Context Menu Overlay (if visible, drawn on top of the rendered scene)
        if context_menu.is_visible {
            context_menu.render(bitmap, vec_buf, font);
            SubpixelRasterizer::render(bitmap, vec_buf, &Transform2D::identity());
            vec_buf.clear();
        }
    }
}

/// `text` cut down to fit `max_w` pixels. A number that cannot fit shows as
/// `####` rather than a misleading prefix of its digits.
fn fit_text(font: &HersheyFont, text: &str, scale: f32, max_w: f32, numeric: bool) -> String {
    if max_w <= 0.0 {
        return String::new();
    }
    let (full, _) = font.measure_string(text, scale);
    if full <= max_w {
        return text.into();
    }
    if numeric {
        let (hash_w, _) = font.measure_string("#", scale);
        let n = if hash_w > 0.0 { (max_w / hash_w) as usize } else { 0 };
        return "#".repeat(n);
    }
    let mut out = String::new();
    let mut used = 0.0;
    for ch in text.chars() {
        let mut buf = [0u8; 4];
        let (w, _) = font.measure_string(ch.encode_utf8(&mut buf), scale);
        if used + w > max_w {
            break;
        }
        used += w;
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Cell;

    #[test]
    fn test_workspace_view_cell_rect_and_hit_test() {
        let mut sheet = Sheet::new("s1", "Main");
        sheet.set_col_width(0, 100.0);
        sheet.set_col_width(1, 120.0);
        sheet.set_row_height(0, 30.0);
        sheet.set_row_height(1, 40.0);

        let view = VectorWorkspaceView::new();

        // Header col = 50.0, Control panel = 48.0, Header row = 24.0 -> grid origin (50.0, 72.0)
        let rect_a1 = view.cell_rect(&sheet, Coord::new(0, 0));
        assert_eq!(rect_a1.min_x, 50.0);
        assert_eq!(rect_a1.min_y, 72.0);
        assert_eq!(rect_a1.width(), 100.0);
        assert_eq!(rect_a1.height(), 30.0);

        let rect_b2 = view.cell_rect(&sheet, Coord::new(1, 1));
        assert_eq!(rect_b2.min_x, 150.0); // 50 + 100
        assert_eq!(rect_b2.min_y, 102.0); // 72 + 30
        assert_eq!(rect_b2.width(), 120.0);
        assert_eq!(rect_b2.height(), 40.0);

        // Hit testing
        assert_eq!(view.hit_test_cell(&sheet, 60.0, 80.0), Some(Coord::new(0, 0)));
        assert_eq!(view.hit_test_cell(&sheet, 160.0, 110.0), Some(Coord::new(1, 1)));
        assert_eq!(view.hit_test_cell(&sheet, 10.0, 10.0), None); // In control panel / headers
    }

    #[test]
    fn test_workspace_view_render_produces_pixels() {
        let mut sheet = Sheet::new("s1", "Main");
        sheet.set_cell(Cell::number(Coord::new(0, 0), 1234.5).with_name("rev"));
        let session = WorkspaceSession::new();
        let context_menu = ContextMenu::new();
        let font = HersheyFont::load_simplex();

        let view = VectorWorkspaceView::new();
        let mut bitmap = Bitmap::new(400, 300);
        let mut vec_buf = VectorBuffer::new();

        view.render(&sheet, &session, &context_menu, &mut bitmap, &mut vec_buf, &font);

        let lit_pixels = bitmap.pixels.iter().filter(|&&p| p != 0).count();
        assert!(lit_pixels > 1000, "Workspace render should illuminate pixels");
    }

    #[test]
    fn test_shift_view_panning() {
        let mut sheet = Sheet::new("s1", "Main");
        sheet.set_col_width(0, 100.0);
        sheet.set_col_width(1, 100.0);
        sheet.set_col_width(2, 100.0);
        sheet.set_row_height(0, 30.0);
        sheet.set_row_height(1, 30.0);

        let mut view = VectorWorkspaceView::new();
        assert_eq!(view.scroll_col, 0);
        assert_eq!(view.scroll_row, 0);

        // Cell C1 at (2, 0): when scroll_col = 0, x = 50 + 100 + 100 = 250
        assert_eq!(view.cell_rect(&sheet, Coord::new(2, 0)).min_x, 250.0);

        // Pan view by 1 column right (Shift+Right)
        view.pan(1, 0);
        assert_eq!(view.scroll_col, 1);
        // Cell C1 now appears at x = 50 + 100 = 150 (shifted left on screen)
        assert_eq!(view.cell_rect(&sheet, Coord::new(2, 0)).min_x, 150.0);

        // Pan view down by 1 row (Shift+Down)
        view.pan(0, 1);
        assert_eq!(view.scroll_row, 1);
        // Cell A2 at (0, 1) now appears at grid origin y = 72
        assert_eq!(view.cell_rect(&sheet, Coord::new(0, 1)).min_y, 72.0);

        // Pan back
        view.pan(-1, -1);
        assert_eq!(view.scroll_col, 0);
        assert_eq!(view.scroll_row, 0);
    }

    // ---- scrolling to the cursor, clipping, status, selection --------------

    #[test]
    fn ensure_visible_scrolls_only_as_far_as_needed() {
        let sheet = Sheet::new("s", "S");
        let mut view = VectorWorkspaceView::new();

        view.ensure_visible(&sheet, Coord::new(2, 3), 800.0, 600.0);
        assert_eq!((view.scroll_col, view.scroll_row), (0, 0), "already visible: no scroll");

        view.ensure_visible(&sheet, Coord::new(30, 0), 800.0, 600.0);
        assert!(view.scroll_col > 0);
        assert!(view.cell_rect(&sheet, Coord::new(30, 0)).max_x <= 800.0);
        assert_eq!(view.cell_rect(&sheet, Coord::new(30, 0)).max_x > 700.0, true, "scrolled just enough, not more");

        view.ensure_visible(&sheet, Coord::new(0, 50), 800.0, 600.0);
        assert!(view.cell_rect(&sheet, Coord::new(0, 50)).max_y <= 600.0);

        view.ensure_visible(&sheet, Coord::new(0, 0), 800.0, 600.0);
        assert_eq!((view.scroll_col, view.scroll_row), (0, 0), "back to the corner");
    }

    #[test]
    fn a_cell_wider_than_the_window_is_aligned_to_its_left_edge() {
        let mut sheet = Sheet::new("s", "S");
        sheet.set_col_width(5, 1_000.0);
        let mut view = VectorWorkspaceView::new();
        view.ensure_visible(&sheet, Coord::new(5, 0), 400.0, 600.0);
        assert_eq!(view.scroll_col, 5);
    }

    #[test]
    fn a_page_is_the_number_of_default_rows_that_fit() {
        let sheet = Sheet::new("s", "S");
        let view = VectorWorkspaceView::new();
        assert_eq!(view.rows_per_page(&sheet, 600.0), 22); // (600 - 48 - 24) / 24
        assert_eq!(view.rows_per_page(&sheet, 10.0), 1, "never zero");
    }

    #[test]
    fn fit_text_clips_text_and_hashes_numbers_that_do_not_fit() {
        let font = HersheyFont::load_simplex();
        let scale = 0.75;
        let (full, _) = font.measure_string("Projection Summary", scale);

        assert_eq!(fit_text(&font, "Projection Summary", scale, full + 1.0, false), "Projection Summary");
        let clipped = fit_text(&font, "Projection Summary", scale, full / 2.0, false);
        assert!(!clipped.is_empty() && clipped.len() < "Projection Summary".len());
        assert!("Projection Summary".starts_with(&clipped));
        assert!(font.measure_string(&clipped, scale).0 <= full / 2.0);

        let hashes = fit_text(&font, "12345678.00", scale, 30.0, true);
        assert!(!hashes.is_empty() && hashes.chars().all(|c| c == '#'), "{hashes:?}");
        assert_eq!(fit_text(&font, "anything", scale, 0.0, false), "");
    }

    /// Only the grid area of a frame (below the 72px control panel), so a test about
    /// what the grid draws is not disturbed by the info line above it.
    fn grid_of(frame: &[u32]) -> Vec<u32> {
        frame[72 * 800..].to_vec()
    }

    fn frame(session: &WorkspaceSession, sheet: &Sheet, view: &VectorWorkspaceView) -> Vec<u32> {
        let font = HersheyFont::load_simplex();
        let mut bitmap = Bitmap::new(800, 600);
        let mut vec_buf = VectorBuffer::with_capacity(1024);
        let menu = ContextMenu::default_context_menu();
        view.render(sheet, session, &menu, &mut bitmap, &mut vec_buf, &font);
        bitmap.pixels.clone()
    }

    #[test]
    fn the_status_message_is_drawn_and_the_hint_returns_when_it_is_cleared() {
        let sheet = Sheet::new("s", "S");
        let view = VectorWorkspaceView::new();
        let mut session = WorkspaceSession::new();
        session.status_message = "READY".into();
        let idle = frame(&session, &sheet, &view);

        session.status_message = "Saved workbook to demo.lotus".into();
        assert!(frame(&session, &sheet, &view) != idle, "the message shows");

        session.status_message = "READY".into();
        assert!(frame(&session, &sheet, &view) == idle, "back to the hint line");
    }

    #[test]
    fn a_selected_range_is_highlighted_behind_the_cells() {
        let sheet = Sheet::new("s", "S");
        let view = VectorWorkspaceView::new();
        let mut session = WorkspaceSession::new();
        session.cursor = Coord::new(2, 4); // C5
        let plain = frame(&session, &sheet, &view);

        session.selection_anchor = Some(Coord::new(3, 5)); // D6
        let selected = frame(&session, &sheet, &view);
        assert!(selected != plain, "selecting changes the picture");

        let r = view.cell_rect(&sheet, Coord::new(3, 5));
        let (x, y) = (((r.min_x + r.max_x) / 2.0) as usize, ((r.min_y + r.max_y) / 2.0) as usize);
        assert_eq!(selected[y * 800 + x], 0x00183050, "the far corner cell is tinted");
        assert!(plain[y * 800 + x] != 0x00183050);
    }

    #[test]
    fn collapsed_rows_and_columns_draw_no_text() {
        let view = VectorWorkspaceView::new();
        let session = WorkspaceSession::new();
        let mut with_text = Sheet::new("s", "S");
        with_text.set_cell(Cell::text(Coord::new(0, 0), "visible"));
        let mut without = Sheet::new("s", "S");
        let grid = |sheet: &Sheet| grid_of(&frame(&session, sheet, &view));

        // sanity: at normal size the text shows up in the grid
        assert!(grid(&with_text) != grid(&without), "text is drawn at normal size");

        // collapse the row: the grid is the same with or without the text
        with_text.set_row_height(0, 8.0);
        without.set_row_height(0, 8.0);
        assert!(grid(&with_text) == grid(&without), "a collapsed row draws no text");

        // and for a collapsed column
        with_text.set_row_height(0, 24.0);
        without.set_row_height(0, 24.0);
        with_text.set_col_width(0, 10.0);
        without.set_col_width(0, 10.0);
        assert!(grid(&with_text) == grid(&without), "a collapsed column draws no text");
    }
}
