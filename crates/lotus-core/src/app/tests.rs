use super::*;

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
    assert!(app.status().starts_with("Pasted into"), "{}", app.status());
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
fn paste_does_not_clobber_an_edit_in_progress() {
    let (mut app, mut host) = new_app();
    type_str(&mut app, &mut host, "7");
    host.clipboard = Some("99".into());
    ctrl(&mut app, &mut host, 'v');
    assert_eq!(app.session.edit_buffer.as_deref(), Some("7"));
    assert!(is_number(value_at(&app, 0, 0), 99.0));
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
    app.handle_event(&mut host, Event::PointerMove { x: r0.min_x + 20.0, y: lotus_y });

    let r1 = app.context_menu.panels()[1].rect;
    app.handle_event(&mut host, Event::PointerMove { x: r1.min_x + 20.0, y: r1.min_y + 4.0 + IH + IH / 2.0 }); // File

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
    app.handle_event(&mut host, Event::PointerMove { x: 5.0, y: 5.0 });
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
