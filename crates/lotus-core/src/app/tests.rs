use super::*;
use crate::calc::CalcEngine;

const IH: f32 = 22.0; // context-menu row height

fn new_app() -> (App, MemoryHost) {
    let mut host = MemoryHost::new();
    let app = App::new(&mut host, 800, 600, None);
    (app, host)
}

fn press(app: &mut App, host: &mut MemoryHost, key: Key) {
    app.handle_event(host, Event::Key { key, mods: Mods::NONE });
}

fn ctrl(app: &mut App, host: &mut MemoryHost, c: char) {
    app.handle_event(host, Event::Key { key: Key::Char(c), mods: Mods::CTRL });
}

fn type_str(app: &mut App, host: &mut MemoryHost, s: &str) {
    for ch in s.chars() {
        app.handle_event(host, Event::Text(ch));
    }
}

fn value_at(app: &App, col: u32, row: u32) -> Option<CellValue> {
    app.workbook.sheets[0].get_cell(Coord::new(col, row)).map(|c| c.value.clone())
}

fn is_number(v: Option<CellValue>, expected: f64) -> bool {
    matches!(v, Some(CellValue::Number(n)) if n == expected)
}

// ---- opening ---------------------------------------------------------------

#[test]
fn starts_with_the_sample_workbook_when_no_file_exists() {
    let (app, _host) = new_app();
    assert_eq!(app.file_path.as_deref(), Some("demo.lotus"));
    assert_eq!(app.workbook.sheets.len(), 1);
    assert!(is_number(value_at(&app, 0, 0), 8000.0));
}

#[test]
fn loads_the_requested_file_through_the_host() {
    let mut host = MemoryHost::new();
    let mut wb = App::sample_workbook();
    wb.title = "Mine".into();
    host.files.insert("my.lotus".into(), serialize_workbook(&wb));

    let app = App::new(&mut host, 800, 600, Some("my.lotus".into()));
    assert_eq!(app.workbook.title, "Mine");
    assert_eq!(app.file_path.as_deref(), Some("my.lotus"));
}

#[test]
fn an_unparseable_file_is_never_overwritten() {
    let mut host = MemoryHost::new();
    host.files.insert("bad.lotus".into(), b"this is not a workbook".to_vec());
    let mut app = App::new(&mut host, 800, 600, Some("bad.lotus".into()));

    assert_eq!(app.file_path, None);
    assert!(app.status().contains("Could not read bad.lotus"), "{}", app.status());

    ctrl(&mut app, &mut host, 's');
    assert!(app.status().starts_with("Not saved"), "{}", app.status());
    assert_eq!(host.files["bad.lotus"], b"this is not a workbook".to_vec());
}

#[test]
fn an_empty_file_counts_as_a_new_one_and_can_be_saved() {
    let mut host = MemoryHost::new();
    host.files.insert("new.lotus".into(), Vec::new());
    let mut app = App::new(&mut host, 800, 600, Some("new.lotus".into()));
    assert_eq!(app.file_path.as_deref(), Some("new.lotus"));

    ctrl(&mut app, &mut host, 's');
    assert!(deserialize_workbook(&host.files["new.lotus"]).is_ok());
    assert!(host.files["new.lotus"].len() > 20);
}

#[test]
fn any_text_file_is_rejected_not_silently_emptied() {
    for junk in [&b"{\"json\": true}"[..], &b"<html><body>hi</body></html>"[..], &b"plain notes"[..]] {
        let mut host = MemoryHost::new();
        host.files.insert("x.lotus".into(), junk.to_vec());
        let app = App::new(&mut host, 800, 600, Some("x.lotus".into()));
        assert_eq!(app.file_path, None, "accepted {:?}", String::from_utf8_lossy(junk));
    }
}

#[test]
fn reloading_a_file_that_stopped_being_a_workbook_keeps_the_open_one() {
    let (mut app, mut host) = new_app();
    ctrl(&mut app, &mut host, 's');
    host.files.insert("demo.lotus".into(), b"oops".to_vec());
    type_str(&mut app, &mut host, "/FR");
    assert!(app.status().starts_with("Reload failed"), "{}", app.status());
    assert!(is_number(value_at(&app, 0, 0), 8000.0));
}

// ---- typing and editing ----------------------------------------------------

#[test]
fn typing_then_enter_commits_a_number() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "42");
    press(&mut app, &mut host, Key::Enter);
    assert!(is_number(value_at(&app, 0, 0), 42.0));
    assert_eq!(app.session.edit_buffer, None);
}

#[test]
fn escape_cancels_an_edit_and_closes_the_context_menu_first() {
    let (mut app, mut host) = new_app();
    app.handle_event(&mut host, Event::PointerDown { x: 100.0, y: 100.0, button: PointerButton::Right });
    assert!(app.context_menu.is_visible);
    type_str(&mut app, &mut host, "x");
    assert!(app.session.edit_buffer.is_some());

    press(&mut app, &mut host, Key::Escape);
    assert!(!app.context_menu.is_visible);
    assert!(app.session.edit_buffer.is_some(), "first Esc only closes the menu");

    press(&mut app, &mut host, Key::Escape);
    assert_eq!(app.session.edit_buffer, None);
}

#[test]
fn left_click_selects_the_cell_under_the_pointer() {
    let (mut app, mut host) = new_app();
    let r = app.view.cell_rect(&app.workbook.sheets[0], Coord::new(1, 1));
    app.handle_event(
        &mut host,
        Event::PointerDown { x: (r.min_x + r.max_x) / 2.0, y: (r.min_y + r.max_y) / 2.0, button: PointerButton::Left },
    );
    assert_eq!(app.session.cursor, Coord::new(1, 1));
}

// ---- clipboard ---------------------------------------------------------------

#[test]
fn copy_and_paste_round_trip_through_the_host_clipboard() {
    let (mut app, mut host) = new_app();
    ctrl(&mut app, &mut host, 'c');
    assert_eq!(host.clipboard.as_deref(), Some("8000"));

    press(&mut app, &mut host, Key::Down);
    ctrl(&mut app, &mut host, 'v');
    assert!(is_number(value_at(&app, 0, 1), 8000.0));
    assert!(app.status().starts_with("Pasted"), "{}", app.status());
}

#[test]
fn pasted_text_is_parsed_like_typed_text() {
    let (mut app, mut host) = new_app();
    host.clipboard = Some("=revenue * 2".into());
    ctrl(&mut app, &mut host, 'v');
    assert!(matches!(value_at(&app, 0, 0), Some(CellValue::Formula { .. })));

    host.clipboard = Some("TRUE".into());
    ctrl(&mut app, &mut host, 'v');
    assert!(matches!(value_at(&app, 0, 0), Some(CellValue::Boolean(true))));
}

#[test]
fn cut_fills_the_clipboard_and_empties_the_cell() {
    let (mut app, mut host) = new_app();
    ctrl(&mut app, &mut host, 'x');
    assert_eq!(host.clipboard.as_deref(), Some("8000"));
    assert!(value_at(&app, 0, 0).is_none());
}

#[test]
fn paste_while_editing_goes_into_the_edit_line_not_the_cell() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "7");
    host.clipboard = Some("99".into());
    ctrl(&mut app, &mut host, 'v');
    assert_eq!(app.session.edit_buffer.as_deref(), Some("799"));
    assert!(is_number(value_at(&app, 0, 0), 8000.0), "the cell is untouched until Enter");
}

#[test]
fn clipboard_text_covers_copyable_values() {
    assert_eq!(cell_clipboard_text(&CellValue::Number(1.5)).as_deref(), Some("1.5"));
    assert_eq!(cell_clipboard_text(&CellValue::Boolean(true)).as_deref(), Some("TRUE"));
    assert_eq!(cell_clipboard_text(&CellValue::Text("hi".into())).as_deref(), Some("hi"));
    assert_eq!(cell_clipboard_text(&CellValue::Text(String::new())), None);
}

// ---- save / reload -----------------------------------------------------------

#[test]
fn slash_menu_save_writes_a_readable_file() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "/FS");
    assert!(app.status().starts_with("Saved workbook to demo.lotus"), "{}", app.status());
    let bytes = host.files.get("demo.lotus").expect("file written through the host");
    assert!(deserialize_workbook(bytes).is_ok());
}

#[test]
fn ctrl_s_saves_and_clears_the_dirty_flag() {
    let (mut app, mut host) = new_app();
    app.session.is_dirty = true;
    ctrl(&mut app, &mut host, 's');
    assert!(host.files.contains_key("demo.lotus"));
    assert!(!app.session.is_dirty);
}

#[test]
fn a_failed_save_says_so_and_stays_dirty() {
    let (mut app, mut host) = new_app();
    host.fail_writes = true;
    app.session.is_dirty = true;
    ctrl(&mut app, &mut host, 's');
    assert!(app.status().contains("Save failed"), "{}", app.status());
    assert!(app.status().contains("disk full"), "{}", app.status());
    assert!(!app.status().contains("Saved workbook"));
    assert!(app.session.is_dirty);
}

#[test]
fn reload_picks_up_changes_made_outside() {
    let (mut app, mut host) = new_app();
    ctrl(&mut app, &mut host, 's');

    let mut other = App::sample_workbook();
    other.title = "Changed elsewhere".into();
    host.files.insert("demo.lotus".into(), serialize_workbook(&other));

    type_str(&mut app, &mut host, "/FR");
    assert_eq!(app.workbook.title, "Changed elsewhere");
    assert!(app.status().starts_with("Reloaded"), "{}", app.status());
}

#[test]
fn a_failed_reload_keeps_the_open_workbook() {
    let (mut app, mut host) = new_app();
    let title = app.workbook.title.clone();
    type_str(&mut app, &mut host, "/FR"); // demo.lotus does not exist yet
    assert!(app.status().starts_with("Reload failed"), "{}", app.status());
    assert_eq!(app.workbook.title, title);
}

// ---- menus -------------------------------------------------------------------

#[test]
fn context_menu_reaches_save_through_three_levels() {
    let (mut app, mut host) = new_app();
    app.handle_event(&mut host, Event::PointerDown { x: 100.0, y: 100.0, button: PointerButton::Right });

    let r0 = app.context_menu.panels()[0].rect;
    let lotus_y = r0.min_y + 4.0 + 4.0 * IH + 6.0 + IH / 2.0; // row 5, after a 6px separator
    app.handle_event(&mut host, Event::PointerMove { x: r0.min_x + 20.0, y: lotus_y, left_down: false });

    let file_row = app.context_menu.items[5].submenus.iter().position(|m| m.label == "File").unwrap() as f32;
    let r1 = app.context_menu.panels()[1].rect;
    app.handle_event(&mut host, Event::PointerMove { x: r1.min_x + 20.0, y: r1.min_y + 4.0 + file_row * IH + IH / 2.0, left_down: false }); // File

    let r2 = app.context_menu.panels()[2].rect;
    app.handle_event(
        &mut host,
        Event::PointerDown { x: r2.min_x + 20.0, y: r2.min_y + 4.0 + IH / 2.0, button: PointerButton::Left }, // Save
    );

    assert!(host.files.contains_key("demo.lotus"), "status: {}", app.status());
    assert!(!app.context_menu.is_visible);
}

#[test]
fn slash_quit_asks_the_host_to_quit() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "/Q");
    assert!(host.quit_requested);
}

#[test]
fn recalculate_and_inspect_report_in_the_status_line() {
    let (mut app, mut host) = new_app();
    app.execute(&mut host, MenuAction::RecalculateAll);
    assert!(app.status().starts_with("Recalculated"), "{}", app.status());

    app.execute(&mut host, MenuAction::FileInspect);
    assert!(app.status().contains("1 sheets"), "{}", app.status());
}

// ---- shell contract ------------------------------------------------------------

#[test]
fn dirty_flag_reports_changes_once() {
    let (mut app, mut host) = new_app();
    assert!(app.take_dirty(), "a fresh app needs a first paint");
    assert!(!app.take_dirty());

    // pointer movement with no menu open changes nothing on screen
    app.handle_event(&mut host, Event::PointerMove { x: 5.0, y: 5.0, left_down: false });
    assert!(!app.take_dirty());

    press(&mut app, &mut host, Key::Down);
    assert!(app.take_dirty());
}

#[test]
fn resize_updates_the_surface_and_menu_viewport() {
    let (mut app, mut host) = new_app();
    app.take_dirty();

    app.handle_event(&mut host, Event::Resize { width: 1024, height: 768 });
    assert_eq!((app.bitmap().width, app.bitmap().height), (1024, 768));
    assert_eq!(app.context_menu.viewport, (1024.0, 768.0));
    assert!(app.take_dirty());

    app.handle_event(&mut host, Event::Resize { width: 0, height: 0 });
    assert_eq!((app.bitmap().width, app.bitmap().height), (1024, 768), "zero-size resize is ignored");
}

#[test]
fn render_produces_a_frame_without_any_os() {
    let (mut app, _host) = new_app();
    let lit = app.render().pixels.iter().filter(|&&p| p != 0).count();
    assert!(lit > 1000, "only {} lit pixels", lit);
}

// ============================================================================
// Spreadsheet keys, selection, undo, prompts, sizes and structure
// ============================================================================

use crate::ops::Range;
use crate::tui::menu::{default_lotus_menu, MenuItem};

fn c(col: u32, row: u32) -> Coord {
    Coord::new(col, row)
}

fn mods(ctrl: bool, shift: bool) -> Mods {
    Mods { ctrl, shift, alt: false }
}

fn key_with(app: &mut App, host: &mut MemoryHost, key: Key, m: Mods) {
    app.handle_event(host, Event::Key { key, mods: m });
}

fn ctrl_shift(app: &mut App, host: &mut MemoryHost, key: Key) {
    key_with(app, host, key, mods(true, true));
}

fn shift(app: &mut App, host: &mut MemoryHost, key: Key) {
    key_with(app, host, key, mods(false, true));
}

/// An app whose sheet is empty, so a test can lay out exactly the cells it needs.
fn blank_app() -> (App, MemoryHost) {
    let (mut app, host) = new_app();
    app.workbook.sheets[0].cells.clear();
    (app, host)
}

fn put(app: &mut App, col: u32, row: u32, input: &str) {
    app.workbook.sheets[0].set_cell(Cell::new(c(col, row), CellValue::parse_input(input)));
    let _ = CalcEngine::recalculate_sheet(&mut app.workbook.sheets[0], None);
}

fn result_at(app: &App, col: u32, row: u32) -> Option<f64> {
    match &app.workbook.sheets[0].get_cell(c(col, row))?.value {
        CellValue::Formula { cached_num, .. } => *cached_num,
        CellValue::Number(n) => Some(*n),
        _ => None,
    }
}

fn expr_at(app: &App, col: u32, row: u32) -> String {
    match &app.workbook.sheets[0].get_cell(c(col, row)).expect("cell").value {
        CellValue::Formula { expr, .. } => expr.clone(),
        other => panic!("not a formula: {other:?}"),
    }
}

fn centre(app: &mut App, col: u32, row: u32) -> (f32, f32) {
    let r = app.view.cell_rect(&app.workbook.sheets[0], c(col, row));
    ((r.min_x + r.max_x) / 2.0, (r.min_y + r.max_y) / 2.0)
}

fn click(app: &mut App, host: &mut MemoryHost, col: u32, row: u32) {
    let (x, y) = centre(app, col, row);
    app.handle_event(host, Event::PointerDown { x, y, button: PointerButton::Left });
    app.handle_event(host, Event::PointerUp { x, y, button: PointerButton::Left });
}

fn drag(app: &mut App, host: &mut MemoryHost, from: (u32, u32), to: (u32, u32)) {
    let (ax, ay) = centre(app, from.0, from.1);
    let (bx, by) = centre(app, to.0, to.1);
    app.handle_event(host, Event::PointerDown { x: ax, y: ay, button: PointerButton::Left });
    app.handle_event(host, Event::PointerMove { x: bx, y: by, left_down: true });
    app.handle_event(host, Event::PointerUp { x: bx, y: by, button: PointerButton::Left });
}

// ---- cursor, view, resize ---------------------------------------------------

#[test]
fn arrows_move_the_cursor_and_the_view_follows_it() {
    let (mut app, mut host) = new_app();
    for _ in 0..40 {
        press(&mut app, &mut host, Key::Down);
    }
    assert_eq!(app.session.cursor, c(0, 40));
    assert!(app.view.scroll_row > 0, "the view scrolled down");
    let r = app.view.cell_rect(&app.workbook.sheets[0], app.session.cursor);
    assert!(r.max_y <= app.bitmap().height as f32, "cursor cell is on screen: {r:?}");

    for _ in 0..40 {
        press(&mut app, &mut host, Key::Up);
    }
    assert_eq!((app.session.cursor, app.view.scroll_row), (c(0, 0), 0));
}

#[test]
fn shift_arrows_move_the_view_not_the_cursor() {
    let (mut app, mut host) = new_app();
    shift(&mut app, &mut host, Key::Down);
    shift(&mut app, &mut host, Key::Down);
    shift(&mut app, &mut host, Key::Right);
    assert_eq!((app.view.scroll_row, app.view.scroll_col), (2, 1));
    assert_eq!(app.session.cursor, c(0, 0));
    shift(&mut app, &mut host, Key::Up);
    shift(&mut app, &mut host, Key::Left);
    assert_eq!((app.view.scroll_row, app.view.scroll_col), (1, 0));
}

#[test]
fn ctrl_shift_arrows_resize_the_cursors_column_and_row() {
    let (mut app, mut host) = new_app();
    ctrl_shift(&mut app, &mut host, Key::Right);
    assert_eq!(app.workbook.sheets[0].col_width(0), 110.0);
    for _ in 0..3 {
        ctrl_shift(&mut app, &mut host, Key::Left);
    }
    assert_eq!(app.workbook.sheets[0].col_width(0), 80.0);
    ctrl_shift(&mut app, &mut host, Key::Down);
    assert_eq!(app.workbook.sheets[0].row_height(0), 28.0);
    ctrl_shift(&mut app, &mut host, Key::Up);
    assert_eq!(app.workbook.sheets[0].row_height(0), 24.0);
    assert_eq!(app.session.cursor, c(0, 0), "resizing does not move the cursor");
    assert_eq!(app.workbook.sheets[0].col_width(1), 100.0, "other columns are untouched");
}

#[test]
fn sizes_have_a_floor_and_a_ceiling() {
    let (mut app, mut host) = new_app();
    for _ in 0..30 {
        ctrl_shift(&mut app, &mut host, Key::Left);
    }
    assert_eq!(app.workbook.sheets[0].col_width(0), 10.0);
    for _ in 0..200 {
        ctrl_shift(&mut app, &mut host, Key::Right);
    }
    assert_eq!(app.workbook.sheets[0].col_width(0), 1_000.0);
}

#[test]
fn a_held_resize_is_a_single_undo_step() {
    let (mut app, mut host) = new_app();
    for _ in 0..5 {
        ctrl_shift(&mut app, &mut host, Key::Right);
    }
    assert_eq!(app.workbook.sheets[0].col_width(0), 150.0);
    ctrl(&mut app, &mut host, 'z');
    assert_eq!(app.workbook.sheets[0].col_width(0), 100.0, "one Ctrl+Z undoes the whole run");
    ctrl(&mut app, &mut host, 'z');
    assert!(app.status().contains("Nothing to undo"), "{}", app.status());
}

#[test]
fn ctrl_arrows_jump_to_the_edge_of_the_data() {
    let (mut app, mut host) = new_app();
    key_with(&mut app, &mut host, Key::Down, mods(true, false));
    assert_eq!(app.session.cursor, c(0, 2), "end of the A1:A3 block");
    key_with(&mut app, &mut host, Key::Down, mods(true, false));
    assert_eq!(app.session.cursor, c(0, 9_999), "then the bottom of the grid");
    key_with(&mut app, &mut host, Key::Up, mods(true, false));
    assert_eq!(app.session.cursor, c(0, 2));
}

#[test]
fn home_end_and_page_keys() {
    let (mut app, mut host) = new_app();
    for _ in 0..4 {
        press(&mut app, &mut host, Key::Down);
    }
    press(&mut app, &mut host, Key::Right);
    press(&mut app, &mut host, Key::Home);
    assert_eq!(app.session.cursor, c(0, 4));
    key_with(&mut app, &mut host, Key::Home, mods(true, false));
    assert_eq!(app.session.cursor, c(0, 0));
    press(&mut app, &mut host, Key::End);
    assert_eq!(app.session.cursor, c(1, 0), "last used column of the row");
    key_with(&mut app, &mut host, Key::End, mods(true, false));
    assert_eq!(app.session.cursor, c(1, 2), "last used cell");

    key_with(&mut app, &mut host, Key::Home, mods(true, false));
    press(&mut app, &mut host, Key::PageDown);
    let page = app.view.rows_per_page(&app.workbook.sheets[0], 600.0);
    assert_eq!(app.session.cursor.row, page);
    assert_eq!(app.view.scroll_row, page);
    press(&mut app, &mut host, Key::PageUp);
    assert_eq!((app.session.cursor.row, app.view.scroll_row), (0, 0));
}

#[test]
fn tab_and_shift_tab_move_sideways() {
    let (mut app, mut host) = new_app();
    press(&mut app, &mut host, Key::Tab);
    press(&mut app, &mut host, Key::Tab);
    assert_eq!(app.session.cursor, c(2, 0));
    key_with(&mut app, &mut host, Key::Tab, mods(false, true));
    assert_eq!(app.session.cursor, c(1, 0));
}

#[test]
fn the_scroll_wheel_pans_the_view() {
    let (mut app, mut host) = new_app();
    app.handle_event(&mut host, Event::Scroll { cols: 1, rows: 3 });
    assert_eq!((app.view.scroll_col, app.view.scroll_row), (1, 3));
    app.handle_event(&mut host, Event::Scroll { cols: -5, rows: -9 });
    assert_eq!((app.view.scroll_col, app.view.scroll_row), (0, 0));
    assert!(app.take_dirty());
}

// ---- collapse, expand, fit, set size ---------------------------------------

#[test]
fn collapse_squeezes_and_expand_restores_the_old_size() {
    let (mut app, mut host) = new_app();
    app.workbook.sheets[0].set_col_width(0, 150.0);
    app.workbook.sheets[0].set_row_height(0, 60.0);

    ctrl(&mut app, &mut host, '0');
    assert_eq!(app.workbook.sheets[0].col_width(0), 10.0);
    key_with(&mut app, &mut host, Key::Char('0'), mods(true, true));
    assert_eq!(app.workbook.sheets[0].col_width(0), 150.0);

    ctrl(&mut app, &mut host, '9');
    assert_eq!(app.workbook.sheets[0].row_height(0), 8.0);
    key_with(&mut app, &mut host, Key::Char('9'), mods(true, true));
    assert_eq!(app.workbook.sheets[0].row_height(0), 60.0);
}

#[test]
fn collapse_and_expand_cover_the_whole_selection_and_undo() {
    let (mut app, mut host) = new_app();
    app.session.selection_anchor = Some(c(2, 0));
    app.session.cursor = c(0, 0);
    ctrl(&mut app, &mut host, '0');
    for col in 0..=2 {
        assert_eq!(app.workbook.sheets[0].col_width(col), 10.0);
    }
    assert!(app.status().contains("Collapsed 3"), "{}", app.status());
    ctrl(&mut app, &mut host, '0');
    assert!(app.status().contains("Already collapsed"), "{}", app.status());

    ctrl(&mut app, &mut host, 'z');
    assert_eq!(app.workbook.sheets[0].col_width(1), 100.0, "undo restores every column");
}

#[test]
fn expanding_without_a_remembered_size_uses_the_default() {
    let (mut app, mut host) = new_app();
    app.workbook.sheets[0].set_col_width(0, 10.0); // made tiny some other way
    key_with(&mut app, &mut host, Key::Char('0'), mods(true, true));
    assert_eq!(app.workbook.sheets[0].col_width(0), 100.0);
    key_with(&mut app, &mut host, Key::Char('0'), mods(true, true));
    assert!(app.status().contains("Not collapsed"), "{}", app.status());
}

#[test]
fn fit_sizes_the_column_to_its_widest_value() {
    let (mut app, mut host) = new_app();
    press(&mut app, &mut host, Key::Right); // column B holds "Projection Summary"
    let scale = app.view.font_scale;
    let expected = ops::fit_col_width(&app.workbook.sheets[0], 1, &|s: &str| app.font.measure_string(s, scale).0);
    assert_ne!(expected, 100.0, "the test needs a column that actually changes");
    type_str(&mut app, &mut host, "/CF");
    assert_eq!(app.workbook.sheets[0].col_width(1), expected);
}

#[test]
fn the_width_prompt_sets_the_width() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "/CW");
    assert_eq!(app.prompt, Some(Prompt::ColumnWidth));
    assert_eq!(app.session.edit_prompt.as_deref(), Some("Column width: "));
    assert_eq!(app.session.edit_buffer.as_deref(), Some("100"), "shows the current width");

    type_str(&mut app, &mut host, "150"); // replaces the selected suggestion
    press(&mut app, &mut host, Key::Enter);
    assert_eq!(app.workbook.sheets[0].col_width(0), 150.0);
    assert_eq!((app.prompt, app.session.edit_buffer.clone()), (None, None));
}

#[test]
fn a_bad_size_is_reported_and_changes_nothing() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "/CW");
    type_str(&mut app, &mut host, "abc");
    press(&mut app, &mut host, Key::Enter);
    assert!(app.status().contains("not a number"), "{}", app.status());
    assert_eq!(app.workbook.sheets[0].col_width(0), 100.0);

    type_str(&mut app, &mut host, "/RH");
    type_str(&mut app, &mut host, "40");
    press(&mut app, &mut host, Key::Enter);
    assert_eq!(app.workbook.sheets[0].row_height(0), 40.0);
}

#[test]
fn escape_cancels_a_prompt() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "/CW");
    type_str(&mut app, &mut host, "999");
    press(&mut app, &mut host, Key::Escape);
    assert_eq!((app.prompt, app.session.edit_prompt.clone(), app.session.edit_buffer.clone()), (None, None, None));
    assert_eq!(app.workbook.sheets[0].col_width(0), 100.0);
}

// ---- find and go to ----------------------------------------------------------

#[test]
fn find_asks_then_jumps_and_f3_repeats() {
    let (mut app, mut host) = new_app();
    ctrl(&mut app, &mut host, 'f');
    assert_eq!(app.session.edit_prompt.as_deref(), Some("Find: "));
    type_str(&mut app, &mut host, "active");
    press(&mut app, &mut host, Key::Enter);
    assert_eq!(app.session.cursor, c(1, 1));
    assert!(app.status().contains("Found"), "{}", app.status());

    press(&mut app, &mut host, Key::F(3));
    assert_eq!(app.session.cursor, c(1, 1), "the only match wraps to itself");

    ctrl(&mut app, &mut host, 'f');
    assert_eq!(app.session.edit_buffer.as_deref(), Some("active"), "the last search is offered");
    type_str(&mut app, &mut host, "zzz");
    press(&mut app, &mut host, Key::Enter);
    assert!(app.status().contains("not found"), "{}", app.status());
    assert_eq!(app.session.cursor, c(1, 1), "a miss leaves the cursor alone");
}

#[test]
fn f3_with_no_earlier_search_asks_what_to_find() {
    let (mut app, mut host) = new_app();
    press(&mut app, &mut host, Key::F(3));
    assert_eq!(app.prompt, Some(Prompt::Find));
}

#[test]
fn go_to_takes_an_address_or_a_name() {
    let (mut app, mut host) = new_app();
    ctrl(&mut app, &mut host, 'g');
    type_str(&mut app, &mut host, "c30");
    press(&mut app, &mut host, Key::Enter);
    assert_eq!(app.session.cursor, c(2, 29));
    let r = app.view.cell_rect(&app.workbook.sheets[0], c(2, 29));
    assert!(r.max_y <= 600.0, "scrolled into view");

    press(&mut app, &mut host, Key::F(5));
    type_str(&mut app, &mut host, "REVENUE");
    press(&mut app, &mut host, Key::Enter);
    assert_eq!(app.session.cursor, c(0, 0));

    press(&mut app, &mut host, Key::F(5));
    type_str(&mut app, &mut host, "nowhere");
    press(&mut app, &mut host, Key::Enter);
    assert!(app.status().contains("No cell or name"), "{}", app.status());
}

#[test]
fn clicking_the_grid_cancels_a_prompt() {
    let (mut app, mut host) = new_app();
    ctrl(&mut app, &mut host, 'f');
    click(&mut app, &mut host, 1, 1);
    assert_eq!((app.prompt, app.session.edit_buffer.clone()), (None, None));
    assert_eq!(app.session.cursor, c(1, 1));
}

// ---- selection -----------------------------------------------------------------

#[test]
fn dragging_selects_a_range_and_copies_it_as_tab_separated_text() {
    let (mut app, mut host) = new_app();
    drag(&mut app, &mut host, (0, 0), (1, 2));
    assert_eq!(app.session.selection(), Some(Range::new(c(0, 0), c(1, 2))));
    assert_eq!(app.session.cursor, c(1, 2));

    ctrl(&mut app, &mut host, 'c');
    assert_eq!(
        host.clipboard.as_deref(),
        Some("8000\tProjection Summary\n0.2\tActive\n=revenue * tax_rate\t")
    );
    assert_eq!(app.status(), "Copied A1:B3");
}

#[test]
fn a_plain_click_selects_one_cell_only() {
    let (mut app, mut host) = new_app();
    click(&mut app, &mut host, 1, 1);
    assert_eq!(app.session.cursor, c(1, 1));
    assert_eq!((app.session.selection(), app.session.selection_anchor), (None, None));
}

#[test]
fn a_drag_ends_when_the_button_was_released_outside_the_window() {
    let (mut app, mut host) = new_app();
    let (ax, ay) = centre(&mut app, 0, 0);
    app.handle_event(&mut host, Event::PointerDown { x: ax, y: ay, button: PointerButton::Left });
    assert!(app.dragging);
    app.handle_event(&mut host, Event::PointerMove { x: ax + 200.0, y: ay + 50.0, left_down: false });
    assert!(!app.dragging);
    assert_eq!(app.session.selection_anchor, None);
}

#[test]
fn right_click_selects_what_you_clicked_unless_it_is_inside_the_selection() {
    let (mut app, mut host) = new_app();
    drag(&mut app, &mut host, (0, 0), (1, 1));

    let (x, y) = centre(&mut app, 1, 1);
    app.handle_event(&mut host, Event::PointerDown { x, y, button: PointerButton::Right });
    assert_eq!(app.session.selection(), Some(Range::new(c(0, 0), c(1, 1))), "inside: selection kept");
    assert!(app.context_menu.is_visible);

    press(&mut app, &mut host, Key::Escape);
    let (x, y) = centre(&mut app, 3, 4);
    app.handle_event(&mut host, Event::PointerDown { x, y, button: PointerButton::Right });
    assert_eq!((app.session.cursor, app.session.selection()), (c(3, 4), None), "outside: moves there");
}

#[test]
fn f8_extends_the_selection_with_the_arrows_and_escape_ends_it() {
    let (mut app, mut host) = new_app();
    press(&mut app, &mut host, Key::F(8));
    assert!(app.session.extend_mode);
    press(&mut app, &mut host, Key::Down);
    press(&mut app, &mut host, Key::Down);
    press(&mut app, &mut host, Key::Right);
    assert_eq!(app.session.selection(), Some(Range::new(c(0, 0), c(1, 2))));
    press(&mut app, &mut host, Key::Escape);
    assert_eq!((app.session.selection(), app.session.extend_mode), (None, false));
}

#[test]
fn plain_arrows_drop_the_selection() {
    let (mut app, mut host) = new_app();
    drag(&mut app, &mut host, (0, 0), (1, 1));
    press(&mut app, &mut host, Key::Right);
    assert_eq!(app.session.selection(), None);
}

#[test]
fn ctrl_a_selects_the_used_range() {
    let (mut app, mut host) = new_app();
    ctrl(&mut app, &mut host, 'a');
    assert_eq!(app.session.selection(), Some(Range::new(c(0, 0), c(1, 2))));
    assert_eq!(app.status(), "Selected A1:B3");
}

// ---- ranges: clipboard, clear, fill, sort ------------------------------------

#[test]
fn copying_and_pasting_a_range_moves_relative_references() {
    let (mut app, mut host) = blank_app();
    put(&mut app, 0, 0, "4");
    put(&mut app, 1, 0, "=A1*3");
    app.session.selection_anchor = Some(c(1, 0));
    app.session.cursor = c(0, 0);
    ctrl(&mut app, &mut host, 'c');
    press(&mut app, &mut host, Key::Escape);

    for _ in 0..4 {
        press(&mut app, &mut host, Key::Down);
    }
    ctrl(&mut app, &mut host, 'v');
    assert_eq!(expr_at(&app, 1, 4), "=A5*3");
    assert_eq!(result_at(&app, 1, 4), Some(12.0));
    assert_eq!(app.session.selection(), Some(Range::new(c(0, 4), c(1, 4))), "the pasted block is selected");
}

#[test]
fn text_from_outside_pastes_exactly_as_written() {
    let (mut app, mut host) = blank_app();
    host.clipboard = Some("=A1*2\t5\n".into());
    press(&mut app, &mut host, Key::Down);
    ctrl(&mut app, &mut host, 'v');
    assert_eq!(expr_at(&app, 0, 1), "=A1*2");
    assert_eq!(result_at(&app, 1, 1), Some(5.0));
}

#[test]
fn delete_clears_the_whole_selection() {
    let (mut app, mut host) = new_app();
    drag(&mut app, &mut host, (0, 0), (1, 1));
    press(&mut app, &mut host, Key::Delete);
    assert_eq!(app.workbook.sheets[0].cells.len(), 1, "only A3 is left");
    assert_eq!(app.status(), "Cleared A1:B2");
}

#[test]
fn cutting_a_range_fills_the_clipboard_and_clears_it() {
    let (mut app, mut host) = new_app();
    drag(&mut app, &mut host, (0, 0), (0, 1));
    ctrl(&mut app, &mut host, 'x');
    assert_eq!(host.clipboard.as_deref(), Some("8000\n0.2"));
    assert!(value_at(&app, 0, 0).is_none() && value_at(&app, 0, 1).is_none());
}

#[test]
fn ctrl_d_and_ctrl_r_fill() {
    let (mut app, mut host) = blank_app();
    put(&mut app, 0, 0, "1");
    put(&mut app, 1, 0, "=A1+1");
    app.session.selection_anchor = Some(c(1, 2));
    app.session.cursor = c(0, 0);
    ctrl(&mut app, &mut host, 'd');
    assert_eq!(expr_at(&app, 1, 2), "=A3+1");
    assert_eq!(result_at(&app, 1, 2), Some(2.0));

    app.session.clear_selection();
    app.session.selection_anchor = Some(c(3, 0));
    app.session.cursor = c(0, 0);
    ctrl(&mut app, &mut host, 'r');
    assert_eq!(result_at(&app, 3, 0), Some(1.0));
}

#[test]
fn fill_from_the_menu_reports_when_there_is_nothing_to_copy_from() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "/DFD"); // Data > Fill > Down on A1 (nothing above)
    assert!(app.status().contains("Nothing above"), "{}", app.status());
    assert!(app.undo.is_empty(), "a failed edit is not an undo step");
}

#[test]
fn sort_orders_the_selected_rows_by_the_cursor_column() {
    let (mut app, mut host) = blank_app();
    for (row, n) in [(0, "3"), (1, "1"), (2, "2")] {
        put(&mut app, 0, row, n);
        put(&mut app, 1, row, &format!("=A{}*10", row + 1));
    }
    app.session.selection_anchor = Some(c(1, 2));
    app.session.cursor = c(0, 0); // column A is the key

    type_str(&mut app, &mut host, "/DSA");
    let a: Vec<_> = (0..3).map(|r| result_at(&app, 0, r).unwrap()).collect();
    assert_eq!(a, vec![1.0, 2.0, 3.0]);
    assert_eq!(result_at(&app, 1, 0), Some(10.0), "same-row formulas follow their row");
    assert!(app.status().contains("ascending"), "{}", app.status());

    type_str(&mut app, &mut host, "/DSD");
    assert_eq!(result_at(&app, 0, 0), Some(3.0));
    ctrl(&mut app, &mut host, 'z');
    assert_eq!(result_at(&app, 0, 0), Some(1.0), "sort is one undo step");
}

#[test]
fn sort_without_a_selection_says_what_to_do() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "/DSA");
    assert!(app.status().contains("Select the rows"), "{}", app.status());
}

// ---- insert and delete -------------------------------------------------------

#[test]
fn inserting_and_deleting_rows_repairs_formulas_and_undoes() {
    let (mut app, mut host) = blank_app();
    put(&mut app, 0, 0, "1");
    put(&mut app, 0, 1, "2");
    put(&mut app, 0, 2, "=A1+A2");
    press(&mut app, &mut host, Key::Down); // cursor on A2

    type_str(&mut app, &mut host, "/RI");
    assert_eq!(expr_at(&app, 0, 3), "=A1+A3");
    assert_eq!(result_at(&app, 0, 3), Some(3.0));
    assert!(app.status().starts_with("Inserted 1 row"), "{}", app.status());

    ctrl(&mut app, &mut host, 'z');
    assert_eq!(expr_at(&app, 0, 2), "=A1+A2");

    type_str(&mut app, &mut host, "/RD"); // delete A2
    assert_eq!(expr_at(&app, 0, 1), "=A1+#REF!");
    assert!(app.status().starts_with("Deleted 1 row"), "{}", app.status());
}

#[test]
fn inserting_a_column_shifts_the_references_across() {
    let (mut app, mut host) = blank_app();
    put(&mut app, 0, 0, "1");
    put(&mut app, 1, 0, "2");
    put(&mut app, 2, 0, "=A1+B1");
    press(&mut app, &mut host, Key::Right); // cursor on B1
    type_str(&mut app, &mut host, "/CI");
    assert_eq!(expr_at(&app, 3, 0), "=A1+C1");
    type_str(&mut app, &mut host, "/CD");
    assert_eq!(expr_at(&app, 2, 0), "=A1+B1");
}

#[test]
fn a_selection_of_several_rows_inserts_that_many() {
    let (mut app, mut host) = blank_app();
    put(&mut app, 0, 0, "1");
    put(&mut app, 0, 1, "2");
    app.session.selection_anchor = Some(c(0, 1));
    app.session.cursor = c(0, 0);
    type_str(&mut app, &mut host, "/RI");
    assert!(app.status().starts_with("Inserted 2 row"), "{}", app.status());
    assert_eq!(result_at(&app, 0, 2), Some(1.0));
}

#[test]
fn an_insert_that_cannot_fit_is_refused_and_leaves_no_undo_step() {
    let (mut app, mut host) = blank_app();
    put(&mut app, 0, 9_999, "1");
    type_str(&mut app, &mut host, "/RI");
    assert!(app.status().contains("pushed past"), "{}", app.status());
    assert_eq!(result_at(&app, 0, 9_999), Some(1.0));
    assert!(app.undo.is_empty());
}

// ---- undo / redo ---------------------------------------------------------------

#[test]
fn undo_and_redo_walk_through_edits() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "42");
    press(&mut app, &mut host, Key::Enter);
    assert!(is_number(value_at(&app, 0, 0), 42.0));

    ctrl(&mut app, &mut host, 'z');
    assert!(is_number(value_at(&app, 0, 0), 8000.0));
    assert_eq!(app.workbook.sheets[0].get_cell(c(0, 0)).unwrap().name.as_deref(), Some("revenue"));
    ctrl(&mut app, &mut host, 'y');
    assert!(is_number(value_at(&app, 0, 0), 42.0));

    ctrl(&mut app, &mut host, 'z');
    type_str(&mut app, &mut host, "7");
    press(&mut app, &mut host, Key::Enter);
    ctrl(&mut app, &mut host, 'y');
    assert!(app.status().contains("Nothing to redo"), "a new edit discards the redo trail: {}", app.status());
}

#[test]
fn undo_reverts_a_clear_a_cut_and_a_paste() {
    let (mut app, mut host) = new_app();
    press(&mut app, &mut host, Key::Delete);
    assert!(value_at(&app, 0, 0).is_none());
    ctrl(&mut app, &mut host, 'z');
    assert!(is_number(value_at(&app, 0, 0), 8000.0));

    host.clipboard = Some("1\t2\n3\t4".into());
    ctrl(&mut app, &mut host, 'v');
    assert!(is_number(value_at(&app, 1, 1), 4.0));
    ctrl(&mut app, &mut host, 'z');
    assert_eq!(app.workbook.sheets[0].get_cell(c(1, 1)).unwrap().value, CellValue::Text("Active".into()));
}

#[test]
fn undo_returns_the_cursor_to_where_the_edit_happened() {
    let (mut app, mut host) = new_app();
    press(&mut app, &mut host, Key::Right);
    press(&mut app, &mut host, Key::Delete);
    for _ in 0..5 {
        press(&mut app, &mut host, Key::Down);
    }
    ctrl(&mut app, &mut host, 'z');
    assert_eq!(app.session.cursor, c(1, 0));
}

#[test]
fn undo_history_is_capped() {
    let (mut app, mut host) = new_app();
    for _ in 0..(UNDO_LIMIT + 20) {
        press(&mut app, &mut host, Key::Delete);
    }
    assert_eq!(app.undo.len(), UNDO_LIMIT);
}

#[test]
fn reloading_forgets_the_undo_history() {
    let (mut app, mut host) = new_app();
    ctrl(&mut app, &mut host, 's');
    press(&mut app, &mut host, Key::Delete);
    assert!(!app.undo.is_empty());
    type_str(&mut app, &mut host, "/FR");
    assert!(app.undo.is_empty() && app.redo.is_empty());
}

// ---- editing like a text editor --------------------------------------------------

#[test]
fn the_edit_line_behaves_like_a_text_editor() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "HELLO WORLD");
    for _ in 0..5 {
        shift(&mut app, &mut host, Key::Left);
    }
    assert_eq!(app.session.selected_text().as_deref(), Some("WORLD"));

    ctrl(&mut app, &mut host, 'c');
    assert_eq!(host.clipboard.as_deref(), Some("WORLD"));
    ctrl(&mut app, &mut host, 'x');
    assert_eq!(app.session.edit_buffer.as_deref(), Some("HELLO "));
    ctrl(&mut app, &mut host, 'v');
    assert_eq!(app.session.edit_buffer.as_deref(), Some("HELLO WORLD"));

    press(&mut app, &mut host, Key::Home);
    press(&mut app, &mut host, Key::Delete);
    assert_eq!(app.session.edit_buffer.as_deref(), Some("ELLO WORLD"));
    key_with(&mut app, &mut host, Key::Right, mods(true, false));
    assert_eq!(app.session.edit_cursor, 5, "Ctrl+Right jumps a word");
    press(&mut app, &mut host, Key::End);
    key_with(&mut app, &mut host, Key::Left, mods(true, false));
    assert_eq!(app.session.edit_cursor, 5, "Ctrl+Left jumps back to the start of the word");

    ctrl(&mut app, &mut host, 'a');
    type_str(&mut app, &mut host, "X");
    assert_eq!(app.session.edit_buffer.as_deref(), Some("X"), "typing replaces the selection");
}

#[test]
fn backspace_and_typing_work_at_the_caret_not_just_the_end() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "abc");
    press(&mut app, &mut host, Key::Left);
    press(&mut app, &mut host, Key::Backspace);
    assert_eq!(app.session.edit_buffer.as_deref(), Some("ac"));
    type_str(&mut app, &mut host, "Z");
    assert_eq!(app.session.edit_buffer.as_deref(), Some("aZc"));
}

#[test]
fn a_slash_inside_text_is_text_not_the_menu() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "a/b");
    assert_eq!(app.session.edit_buffer.as_deref(), Some("a/b"));
    assert!(!app.session.menu.is_active);
}

#[test]
fn arrows_and_tab_commit_the_edit_and_move() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "5");
    press(&mut app, &mut host, Key::Down);
    assert!(is_number(value_at(&app, 0, 0), 5.0));
    assert_eq!((app.session.cursor, app.session.edit_buffer.clone()), (c(0, 1), None));

    type_str(&mut app, &mut host, "6");
    press(&mut app, &mut host, Key::Tab);
    assert!(is_number(value_at(&app, 0, 1), 6.0));
    assert_eq!(app.session.cursor, c(1, 1));
}

#[test]
fn f2_edits_the_current_cell_in_place() {
    let (mut app, mut host) = new_app();
    press(&mut app, &mut host, Key::F(2));
    assert_eq!(app.session.edit_buffer.as_deref(), Some("8000"));
    assert_eq!(app.session.edit_cursor, 4, "caret at the end");
}

// ---- the menu tree ------------------------------------------------------------------

#[test]
fn every_menu_command_runs_without_panicking() {
    fn leaves(items: &[MenuItem], out: &mut Vec<MenuAction>) {
        for i in items {
            if i.submenus.is_empty() {
                out.push(i.action.clone());
            } else {
                leaves(&i.submenus, out);
            }
        }
    }
    let mut all = Vec::new();
    leaves(&default_lotus_menu(), &mut all);
    assert!(all.len() >= 33, "the menu has grown to {} commands", all.len());

    for action in all {
        let (mut app, mut host) = new_app();
        app.session.selection_anchor = Some(c(1, 2)); // give range commands a range
        app.execute(&mut host, action.clone());
        press(&mut app, &mut host, Key::Escape); // dismiss any prompt the command opened
        assert!(app.prompt.is_none(), "{action:?} left a prompt open");
    }
}

#[test]
fn quit_still_works_from_the_longer_menu() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "/Q");
    assert!(host.quit_requested);
}
