use lotus_core::{
    deserialize_workbook, serialize_workbook, CalcEngine, Cell, CellValue, Coord, Sheet, Workbook,
};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Color, ContentStyle, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "demo" => run_demo(),
        "inspect" => {
            if args.len() < 3 {
                eprintln!("Error: Missing file path for inspect");
                return;
            }
            inspect_file(&args[2]);
        }
        "history" => {
            if args.len() < 3 {
                eprintln!("Usage: lotus history <file.lotus> [sheet_id]");
                return;
            }
            inspect_history(&args[2], args.get(3).map(String::as_str));
        }
        "view-z" => {
            if args.len() < 5 {
                eprintln!("Usage: lotus view-z <file.lotus> <sheet_id> <z>");
                return;
            }
            view_history_layer(&args[2], &args[3], &args[4]);
        }
        "view" => {
            if args.len() < 3 {
                eprintln!("Error: Missing file path for view");
                return;
            }
            view_sheet(&args[2]);
        }
        "screen" => {
            if args.len() < 3 {
                eprintln!("Error: Missing file path for screen");
                return;
            }
            render_screen_cli(&args[2]);
        }
        "workspace" => {
            if args.len() < 3 {
                eprintln!("Error: Missing file path for workspace");
                return;
            }
            if let Err(err) = run_interactive_workspace(&args[2]) {
                eprintln!("Workspace error: {}", err);
            }
        }
        "doc" => {
            if args.len() < 3 {
                eprintln!("Usage: lotus doc <file.lotus> [document_id]");
                return;
            }
            let doc_id = args.get(3).map(|s| s.as_str());
            view_document_cli(&args[2], doc_id);
        }
        "set" => {
            if args.len() < 5 {
                eprintln!("Usage: lotus set <file.lotus> <coord_or_name> <value_or_formula>");
                return;
            }
            set_cell_cli(&args[2], &args[3], &args[4]);
        }
        "import" => {
            if args.len() < 6 {
                eprintln!("Usage: lotus import <file.lotus> <coord> <name> <file_to_import>");
                return;
            }
            import_cell_cli(&args[2], &args[3], &args[4], &args[5]);
        }
        "recalc" => {
            if args.len() < 3 {
                eprintln!("Error: Missing file path for recalc");
                return;
            }
            recalc_file(&args[2]);
        }
        "run-basic" => {
            if args.len() < 4 {
                eprintln!("Usage: lotus run-basic <file.lotus> <script_cell_name> [--read A1,A2] [--write A3]");
                return;
            }
            run_basic_cell(&args[2], &args[3], &args[4..]);
        }
        "eject" => {
            if args.len() < 5 {
                eprintln!("Usage: lotus eject <file.lotus> <cell_name> <output_path>");
                return;
            }
            eject_named_cell(&args[2], &args[3], &args[4]);
        }
        _ => print_usage(),
    }
}

fn print_usage() {
    println!("Lotus 5-7-11 CLI — Computational Workspace Harness");
    println!("Usage:");
    println!("  lotus demo                                   Create sample workbook 'demo.lotus'");
    println!("  lotus view <file.lotus>                      View visual spreadsheet grid");
    println!("  lotus screen <file.lotus>                    Render full interactive workspace screen");
    println!("  lotus workspace <file.lotus>                 Open interactive keyboard workspace");
    println!("  lotus doc <file.lotus> [id]                  View embedded live Markdown documents");
    println!("  lotus inspect <file.lotus>                   Inspect workbook structure & TOC");
    println!("  lotus history <file.lotus> [sheet_id]        List stored Z-layer changes");
    println!("  lotus view-z <file.lotus> <sheet_id> <z>     View the composed workbook at a Z layer");
    println!("  lotus set <file.lotus> <coord/name> <val>    Set cell value or formula and recalc");
    println!("  lotus recalc <file.lotus>                    Recalculate formulas and update file");
    println!("  lotus run-basic <file.lotus> <cell_name> [grants] Execute embedded BASIC script (default: owning cell only)");
    println!("  lotus eject <file.lotus> <name> <out>        Eject cell payload to file");
    println!("  lotus import <file.lotus> <coord> <name> <f> Import file into named cell payload");
}

fn run_interactive_workspace(path: &str) -> io::Result<()> {
    let data = fs::read(path)?;
    let initial_file_hash = file_hash(&data);
    let mut wb = deserialize_workbook(&data)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid workbook: {:?}", err)))?;
    if wb.sheets.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Workbook contains no sheets"));
    }

    let mut session = lotus_core::WorkspaceSession::new();
    let mut active_sheet_idx = 0usize;
    let mut historical_z = None;
    let mut basic_choices = Vec::new();
    let mut selected_basic = None;
    let mut reload_pending = false;
    let mut quit_pending = false;
    let mut known_file_hash = initial_file_hash;
    let mut observed_file_hash = initial_file_hash;
    let mut fast_poll_until: Option<Instant> = None;
    let mut next_file_poll = Instant::now() + Duration::from_secs(30);
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    if let Err(err) = execute!(stdout, EnterAlternateScreen, Hide) {
        let _ = terminal::disable_raw_mode();
        return Err(err);
    }

    let result = (|| -> io::Result<()> {
        loop {
            let now = Instant::now();
            if now >= next_file_poll {
                if fast_poll_until.is_some_and(|deadline| now >= deadline) {
                    fast_poll_until = None;
                }
                match poll_external_workbook(
                    path,
                    &mut wb,
                    &mut known_file_hash,
                    &mut observed_file_hash,
                ) {
                    Ok(Some(message)) => session.status_message = message,
                    Ok(None) => {}
                    Err(err) => session.status_message = format!("Workbook poll failed: {}", err),
                }
                next_file_poll = Instant::now() + file_poll_interval(fast_poll_until, Instant::now());
            }

            let mut view_wb = compose_current_workbook(&wb);
            if let Some(z) = historical_z {
                let sheet_id = &wb.sheets[active_sheet_idx].id;
                if let Some(sheet) = wb.sheet_at_z(sheet_id, z) {
                    view_wb.sheets[active_sheet_idx] = sheet;
                }
            }
            if let Err(err) = CalcEngine::recalculate_workbook(&mut view_wb) {
                session.status_message = format!("Recalculation warning: {:?}", err);
            }
            if let Some(z) = historical_z {
                let max_z = wb.history_for_sheet(&wb.sheets[active_sheet_idx].id)
                    .map(|history| history.max_z)
                    .unwrap_or(0);
                if !session.status_message.starts_with("Received ")
                    && !session.status_message.starts_with("External file")
                {
                    session.status_message = format!("HISTORY: viewing Z={} of {} (read-only)", z, max_z);
                }
            }
            let toc = view_wb.compute_toc();
            let (terminal_width, terminal_height) = terminal::size().unwrap_or((80, 24));
            let (viewport_cols, viewport_rows) = viewport_dimensions(terminal_width, terminal_height);
            let sheet = &view_wb.sheets[active_sheet_idx];
            execute!(
                stdout,
                SetBackgroundColor(lotus_background()),
                SetForegroundColor(lotus_text()),
                Clear(ClearType::All),
                MoveTo(0, 0)
            )?;
            let rendered = session.render_workspace_with_viewport(
                sheet,
                &toc,
                viewport_cols,
                viewport_rows,
            );
            let colored = lotus_colorize_frame(&rendered, session.cursor, viewport_cols, viewport_rows);
            write!(stdout, "{}", terminal_line_endings(&colored))?;
            let hint = if selected_basic.is_some() {
                "BASIC: Left/Right choose | Enter runs | Esc cancels"
            } else if historical_z.is_some() {
                "Keys: [ older Z | ] newer Z | 0 latest | arrows move | Tab sheets | q quits"
            } else {
                "Keys: arrows move | Enter edits | [ history | Tab/Shift-Tab sheets | / menu | Ctrl-S saves | q quits"
            };
            write!(stdout, "\r\n{}\r\n", lotus_style(lotus_text(), lotus_background()).apply(hint))?;
            stdout.flush()?;

            let timeout = next_file_poll.saturating_duration_since(Instant::now());
            if !event::poll(timeout)? {
                continue;
            }
            let key = match event::read()? {
                Event::Resize(_, _) => continue,
                Event::Key(key) if key.kind == crossterm::event::KeyEventKind::Press => key,
                _ => continue,
            };

            if reload_pending {
                reload_pending = false;
                if matches!(key.code, KeyCode::Char('y' | 'Y')) {
                    reload_workbook(path, &mut wb, &mut active_sheet_idx, &mut session)?;
                    let bytes = fs::read(path)?;
                    known_file_hash = file_hash(&bytes);
                    observed_file_hash = known_file_hash;
                    historical_z = None;
                } else {
                    session.status_message = "Reload cancelled; workspace changes kept.".into();
                }
                continue;
            }
            if quit_pending {
                quit_pending = false;
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }

            if session.edit_buffer.is_some() {
                if key.code == KeyCode::Enter {
                    let sheet_id = view_wb.sheets[active_sheet_idx].id.clone();
                    let before = view_wb.sheets[active_sheet_idx].clone();
                    handle_edit_key(&mut session, &mut view_wb.sheets[active_sheet_idx], key);
                    let changes = sheet_changes(&before, &view_wb.sheets[active_sheet_idx]);
                    if !changes.is_empty() {
                        match wb.record_history_layer(&sheet_id, changes) {
                            Ok(z) => { session.is_dirty = true; session.status_message = format!("Cell {} recorded at Z={}. Save to keep it.", session.cursor, z); },
                            Err(err) => session.status_message = format!("Unable to record cell change: {}", err),
                        }
                    }
                } else {
                    handle_edit_key(&mut session, &mut view_wb.sheets[active_sheet_idx], key);
                }
                continue;
            }

            if !session.menu.is_active && selected_basic.is_none() {
                let sheet_id = &wb.sheets[active_sheet_idx].id;
                let max_z = wb.history_for_sheet(sheet_id).map(|history| history.max_z).unwrap_or(0);
                match key.code {
                    KeyCode::Char('[') => {
                        if max_z == 0 {
                            session.status_message = "No stored history for this sheet.".into();
                        } else {
                            let current = historical_z.unwrap_or(max_z);
                            if current == 0 {
                                session.status_message = "Already at the original Z=0 layer.".into();
                            } else {
                                historical_z = Some(older_history_z(current));
                            }
                        }
                        continue;
                    }
                    KeyCode::Char(']') => {
                        if let Some(current) = historical_z {
                            historical_z = newer_history_z(current, max_z);
                        } else if max_z == 0 {
                            session.status_message = "No stored history for this sheet.".into();
                        }
                        continue;
                    }
                    KeyCode::Char('0') if historical_z.is_some() => {
                        historical_z = None;
                        continue;
                    }
                    KeyCode::Char('/') if historical_z.is_some() => {
                        session.status_message = "Historical views are read-only; return to latest Z to use the menu.".into();
                        continue;
                    }
                    KeyCode::Enter if historical_z.is_some() => {
                        session.status_message = "Historical views are read-only; return to latest Z to edit.".into();
                        continue;
                    }
                    _ => {}
                }
            }

            if let Some(selected) = selected_basic {
                match key.code {
                    KeyCode::Left if !basic_choices.is_empty() => {
                        let next = if selected == 0 { basic_choices.len() - 1 } else { selected - 1 };
                        selected_basic = Some(next);
                        update_basic_selection_status(&mut session, &basic_choices, next);
                    }
                    KeyCode::Right if !basic_choices.is_empty() => {
                        let next = (selected + 1) % basic_choices.len();
                        selected_basic = Some(next);
                        update_basic_selection_status(&mut session, &basic_choices, next);
                    }
                    KeyCode::Enter if !basic_choices.is_empty() => {
                        let choice = basic_choices[selected].clone();
                        let sheet_id = view_wb.sheets[active_sheet_idx].id.clone();
                        let before = view_wb.sheets[active_sheet_idx].clone();
                        let (declared_reads, declared_writes) = match declared_basic_grants(&view_wb.sheets[active_sheet_idx], &choice.reads, &choice.writes) {
                            Ok(grants) => grants,
                            Err(err) => { session.status_message = format!("Invalid grants for '{}': {}", choice.name, err); selected_basic = None; continue; }
                        };
                        let result = lotus_core::BasicRunner::execute(
                            &mut view_wb.sheets[active_sheet_idx],
                            choice.coord,
                            &choice.source,
                            &declared_reads,
                            &declared_writes,
                            Some(100_000),
                        );
                        session.status_message = match result {
                            Ok(count) => {
                                let changes = sheet_changes(&before, &view_wb.sheets[active_sheet_idx]);
                                if changes.is_empty() {
                                    format!("BASIC '{}' completed: no cell changes.", choice.name)
                                } else {
                                    match wb.record_history_layer(&sheet_id, changes) {
                                        Ok(z) => { session.is_dirty = true; format!("BASIC '{}' completed: {} writes at Z={}. Save to keep it.", choice.name, count, z) },
                                        Err(err) => format!("BASIC '{}' completed, but its layer failed: {}", choice.name, err),
                                    }
                                }
                            }
                            Err(err) => format!("BASIC '{}' error: {:?}", choice.name, err),
                        };
                        selected_basic = None;
                    }
                    KeyCode::Esc => {
                        selected_basic = None;
                        session.status_message = "BASIC selection cancelled.".into();
                    }
                    _ => {}
                }
                continue;
            }

            if session.menu.is_active {
                let action = match key.code {
                    KeyCode::Left => session.menu.handle_key(None, true, false, false, false),
                    KeyCode::Right => session.menu.handle_key(None, false, true, false, false),
                    KeyCode::Up | KeyCode::Esc => session.menu.handle_key(None, false, false, false, true),
                    KeyCode::Down | KeyCode::Enter => session.menu.handle_key(None, false, false, true, false),
                    KeyCode::Char(ch) => session.menu.handle_key(Some(ch), false, false, false, false),
                    _ => lotus_core::MenuAction::None,
                };
                if action == lotus_core::MenuAction::BasicRun {
                    basic_choices = collect_basic_choices(&view_wb.sheets[active_sheet_idx]);
                    if basic_choices.is_empty() {
                        session.status_message = format!("No BASIC procedures on sheet '{}'.", wb.sheets[active_sheet_idx].name);
                    } else {
                        selected_basic = Some(0);
                        update_basic_selection_status(&mut session, &basic_choices, 0);
                    }
                } else if handle_menu_action(
                    action,
                    path,
                    &mut wb,
                    &mut view_wb,
                    &mut session,
                    &mut active_sheet_idx,
                    &mut reload_pending,
                    &mut known_file_hash,
                    &mut observed_file_hash,
                    &mut fast_poll_until,
                    &mut next_file_poll,
                )? {
                    break;
                }
                continue;
            }

            match key.code {
                KeyCode::Char('/') => session.menu.open(),
                KeyCode::Char('q') if session.is_dirty => {
                    session.status_message = "Unsaved Z layers. Press q again to quit; any other key stays.".into();
                    quit_pending = true;
                }
                KeyCode::Char('q') => break,
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let notice = poll_external_workbook(
                        path,
                        &mut wb,
                        &mut known_file_hash,
                        &mut observed_file_hash,
                    )?;
                    if known_file_hash != observed_file_hash {
                        session.status_message = notice.unwrap_or_else(|| "External file changed outside valid Z-layer updates; save blocked to protect it. Reload from the / File menu after resolving the file.".into());
                    } else {
                        let saved_hash = save_workbook(path, &wb)?;
                        known_file_hash = saved_hash;
                        observed_file_hash = saved_hash;
                        session.is_dirty = false;
                        session.status_message = notice.map_or_else(
                            || format!("Saved {}", path),
                            |message| format!("{} Saved {}", message, path),
                        );
                        fast_poll_until = Some(Instant::now() + Duration::from_secs(60));
                        next_file_poll = Instant::now() + Duration::from_secs(10);
                    }
                }
                KeyCode::Enter => session.start_editing(&view_wb.sheets[active_sheet_idx]),
                KeyCode::Left => move_workspace_cursor(&mut session, -1, 0),
                KeyCode::Right => move_workspace_cursor(&mut session, 1, 0),
                KeyCode::Up => move_workspace_cursor(&mut session, 0, -1),
                KeyCode::Down => move_workspace_cursor(&mut session, 0, 1),
                KeyCode::Tab => {
                    active_sheet_idx = next_sheet_index(active_sheet_idx, wb.sheets.len(), 1);
                    historical_z = None;
                    session.status_message = format!("Sheet: {}", view_wb.sheets[active_sheet_idx].name);
                }
                KeyCode::BackTab => {
                    active_sheet_idx = next_sheet_index(active_sheet_idx, wb.sheets.len(), -1);
                    historical_z = None;
                    session.status_message = format!("Sheet: {}", view_wb.sheets[active_sheet_idx].name);
                }
                _ => {}
            }
        }
        Ok(())
    })();

    let screen_restore = execute!(stdout, ResetColor, Show, LeaveAlternateScreen);
    let raw_mode_restore = terminal::disable_raw_mode();
    result.and(screen_restore).and(raw_mode_restore)
}

fn viewport_dimensions(terminal_width: u16, terminal_height: u16) -> (u32, u32) {
    let cols = (terminal_width.saturating_sub(10) / 15).max(1).min(26) as u32;
    // Reserve room for the title, status/edit preview, grid headings, blank
    // separators, and the key-hint footer so the terminal does not scroll them
    // off the top of the alternate screen.
    let rows = terminal_height.saturating_sub(8).max(1) as u32;
    (cols, rows)
}

fn terminal_line_endings(text: &str) -> String {
    text.replace('\n', "\r\n")
}

fn lotus_background() -> Color {
    Color::Rgb { r: 208, g: 208, b: 208 }
}

fn lotus_text() -> Color {
    Color::Rgb { r: 24, g: 24, b: 24 }
}

fn lotus_style(foreground: Color, background: Color) -> ContentStyle {
    let mut style = ContentStyle::new();
    style.foreground_color = Some(foreground);
    style.background_color = Some(background);
    style
}

fn lotus_colorize_frame(text: &str, cursor: Coord, viewport_cols: u32, viewport_rows: u32) -> String {
    let cols = viewport_cols.max(1);
    let rows = viewport_rows.max(1);
    let start_col = cursor.col.saturating_sub(cols / 2);
    let start_row = cursor.row.saturating_sub(rows / 2);
    let row_header_width = format!("{}", start_row.saturating_add(rows)).len().max(3) + 1;
    let cursor_cell_start = row_header_width + 2 + cursor.col.saturating_sub(start_col) as usize * 15;
    let cursor_line = 5 + cursor.row.saturating_sub(start_row) as usize;

    let base = lotus_style(lotus_text(), lotus_background());
    let blue = lotus_style(Color::Blue, lotus_background());
    let divider = lotus_style(Color::DarkGrey, lotus_background());
    let selected = lotus_style(Color::White, Color::Blue);

    let mut out = String::new();
    for (line_index, raw_line) in text.split_inclusive('\n').enumerate() {
        let line = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let style = match line_index {
            1 if line.starts_with("EDIT:") || line.starts_with("MENU:") => blue,
            3 => blue,
            4 => divider,
            _ => base,
        };
        if line_index == cursor_line {
            let chars: Vec<char> = line.chars().collect();
            let start = cursor_cell_start.min(chars.len());
            let end = (start + 15).min(chars.len());
            out.push_str(&format!("{}", style.apply(chars[..start].iter().collect::<String>())));
            out.push_str(&format!("{}", selected.apply(chars[start..end].iter().collect::<String>())));
            out.push_str(&format!("{}", style.apply(chars[end..].iter().collect::<String>())));
        } else {
            out.push_str(&format!("{}", style.apply(line)));
        }
        if raw_line.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

fn handle_edit_key(session: &mut lotus_core::WorkspaceSession, sheet: &mut Sheet, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => session.commit_editing(sheet),
        KeyCode::Esc => session.cancel_editing(),
        KeyCode::Backspace => {
            if let Some(buf) = session.edit_buffer.as_mut() {
                buf.pop();
            }
        }
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(buf) = session.edit_buffer.as_mut() {
                buf.push(ch);
            }
        }
        _ => {}
    }
}

fn move_workspace_cursor(session: &mut lotus_core::WorkspaceSession, dx: i32, dy: i32) {
    session.move_cursor(dx, dy);
}

#[derive(Clone)]
struct BasicChoice {
    coord: Coord,
    name: String,
    source: String,
    reads: Vec<String>,
    writes: Vec<String>,
}

fn collect_basic_choices(sheet: &Sheet) -> Vec<BasicChoice> {
    sheet.cells.values().filter_map(|cell| {
        if let CellValue::Basic { source } = &cell.value {
            Some(BasicChoice {
                coord: cell.coord,
                name: cell.name.clone().unwrap_or_else(|| cell.coord.to_a1()),
                source: source.clone(),
                reads: cell.basic_reads.clone(),
                writes: cell.basic_writes.clone(),
            })
        } else {
            None
        }
    }).collect()
}

fn update_basic_selection_status(session: &mut lotus_core::WorkspaceSession, choices: &[BasicChoice], selected: usize) {
    if let Some(choice) = choices.get(selected) {
        session.cursor = choice.coord;
        session.status_message = format!(
            "BASIC {}/{}: {} at {} — Left/Right choose, Enter run, Esc cancel",
            selected + 1,
            choices.len(),
            choice.name,
            choice.coord,
        );
    }
}

fn next_sheet_index(current: usize, count: usize, direction: i32) -> usize {
    if count == 0 {
        return 0;
    }
    if direction < 0 {
        if current == 0 { count - 1 } else { current - 1 }
    } else {
        (current + 1) % count
    }
}

fn older_history_z(current_z: u32) -> u32 {
    current_z.saturating_sub(1)
}

fn newer_history_z(current_z: u32, max_z: u32) -> Option<u32> {
    let next_z = current_z.saturating_add(1);
    if next_z >= max_z { None } else { Some(next_z) }
}

fn file_hash(bytes: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn file_poll_interval(fast_poll_until: Option<Instant>, now: Instant) -> Duration {
    if fast_poll_until.is_some_and(|deadline| now < deadline) {
        Duration::from_secs(10)
    } else {
        Duration::from_secs(30)
    }
}

fn poll_external_workbook(
    path: &str,
    current: &mut Workbook,
    known_file_hash: &mut u64,
    observed_file_hash: &mut u64,
) -> io::Result<Option<String>> {
    let bytes = fs::read(path)?;
    let incoming_hash = file_hash(&bytes);
    if incoming_hash == *observed_file_hash {
        return Ok(None);
    }
    *observed_file_hash = incoming_hash;

    let incoming = match deserialize_workbook(&bytes) {
        Ok(workbook) => workbook,
        Err(error) => {
            return Ok(Some(format!(
                "External file update is incomplete or invalid ({:?}); keeping current workbook.",
                error
            )));
        }
    };

    match append_external_layers(current, &incoming) {
        Ok(0) => {
            *known_file_hash = incoming_hash;
            Ok(None)
        }
        Ok(count) => {
            *known_file_hash = incoming_hash;
            let noun = if count == 1 { "layer" } else { "layers" };
            Ok(Some(format!("Received {} new Z {} from the workbook file.", count, noun)))
        }
        Err(reason) => Ok(Some(format!(
            "External file update was not imported: {}. Saving is blocked until it is reloaded or corrected.",
            reason
        ))),
    }
}

fn append_external_layers(current: &mut Workbook, incoming: &Workbook) -> Result<usize, String> {
    if current.id != incoming.id
        || current.title != incoming.title
        || current.calc_mode != incoming.calc_mode
        || current.sheets != incoming.sheets
        || current.documents != incoming.documents
    {
        return Err("only append-only Z-layer updates are accepted automatically".into());
    }

    let mut pending = Vec::new();
    for (sheet_id, incoming_history) in &incoming.history {
        let current_history = current.history.get(sheet_id);
        let current_max = current_history.map(|history| history.max_z).unwrap_or(0);
        let overlapping_max = current_max.min(incoming_history.max_z);
        let mut shared_max = 0;

        for z in 1..=overlapping_max {
            let current_layer = current_history.and_then(|history| history.layers.get(&z));
            let incoming_layer = incoming_history.layers.get(&z);
            if current_layer == incoming_layer {
                shared_max = z;
            } else {
                break;
            }
        }

        if incoming_history.max_z > shared_max {
            for z in shared_max + 1..=incoming_history.max_z {
                let records = incoming_history.layers.get(&z)
                    .ok_or_else(|| format!("sheet '{}' is missing Z={} in its history", sheet_id, z))?;
                let changes = records.iter().map(|record| (record.coord, record.cell.clone())).collect();
                pending.push((sheet_id.clone(), changes));
            }
        }
    }

    let count = pending.len();
    for (sheet_id, changes) in pending {
        current.record_history_layer(&sheet_id, changes)
            .map_err(|error| format!("could not append sheet '{}' history: {}", sheet_id, error))?;
    }
    Ok(count)
}

fn save_workbook(path: &str, wb: &Workbook) -> io::Result<u64> {
    let bytes = serialize_workbook(wb);
    fs::write(path, &bytes)?;
    Ok(file_hash(&bytes))
}

fn compose_current_workbook(base: &Workbook) -> Workbook {
    let mut view = base.clone();
    for idx in 0..base.sheets.len() {
        let sheet_id = &base.sheets[idx].id;
        let z = base.history_for_sheet(sheet_id).map(|history| history.max_z).unwrap_or(0);
        if let Some(sheet) = base.sheet_at_z(sheet_id, z) {
            view.sheets[idx] = sheet;
        }
    }
    view
}

fn sheet_changes(before: &Sheet, after: &Sheet) -> Vec<(Coord, Option<Cell>)> {
    let coords: BTreeSet<Coord> = before.cells.keys().chain(after.cells.keys()).copied().collect();
    coords.into_iter().filter_map(|coord| {
        let old = before.cells.get(&coord);
        let new = after.cells.get(&coord);
        let same = match (old, new) {
            (Some(old), Some(new)) => same_cell_definition(old, new),
            (None, None) => true,
            _ => false,
        };
        if same { None } else { Some((coord, new.cloned())) }
    }).collect()
}

fn same_cell_definition(left: &Cell, right: &Cell) -> bool {
    if left.coord != right.coord || left.id != right.id || left.name != right.name {
        return false;
    }
    match (&left.value, &right.value) {
        (CellValue::Formula { expr: left, .. }, CellValue::Formula { expr: right, .. }) => left == right,
        (left, right) => left == right,
    }
}

fn handle_menu_action(
    action: lotus_core::MenuAction,
    path: &str,
    wb: &mut Workbook,
    view_wb: &mut Workbook,
    session: &mut lotus_core::WorkspaceSession,
    active_sheet_idx: &mut usize,
    reload_pending: &mut bool,
    known_file_hash: &mut u64,
    observed_file_hash: &mut u64,
    fast_poll_until: &mut Option<Instant>,
    next_file_poll: &mut Instant,
) -> io::Result<bool> {
    use lotus_core::MenuAction;
    match action {
        MenuAction::None => {}
        MenuAction::Quit => return Ok(true),
        MenuAction::FileSave => {
            let notice = poll_external_workbook(path, wb, known_file_hash, observed_file_hash)?;
            if known_file_hash != observed_file_hash {
                session.status_message = notice.unwrap_or_else(|| "External file update conflicts with the workbook; save blocked to protect it.".into());
            } else {
                let saved_hash = save_workbook(path, wb)?;
                *known_file_hash = saved_hash;
                *observed_file_hash = saved_hash;
                session.is_dirty = false;
                session.status_message = notice.map_or_else(
                    || format!("Saved {}", path),
                    |message| format!("{} Saved {}", message, path),
                );
                *fast_poll_until = Some(Instant::now() + Duration::from_secs(60));
                *next_file_poll = Instant::now() + Duration::from_secs(10);
            }
        }
        MenuAction::FileReload => {
            if session.is_dirty {
                *reload_pending = true;
                session.status_message = "Unsaved Z layers. Reload from disk? Press y to discard them, any other key cancels.".into();
                return Ok(false);
            }
            reload_workbook(path, wb, active_sheet_idx, session)?;
            let bytes = fs::read(path)?;
            *known_file_hash = file_hash(&bytes);
            *observed_file_hash = *known_file_hash;
        }
        MenuAction::FileInspect => {
            let toc = view_wb.compute_toc();
            session.status_message = format!("{} sheets, {} named entries; TOC at {}", view_wb.sheets.len(), toc.entries.len(), toc.pos);
        }
        MenuAction::RecalculateAll => {
            let count = CalcEngine::recalculate_workbook(view_wb).map_err(|err| {
                io::Error::new(io::ErrorKind::Other, format!("Recalculation failed: {:?}", err))
            })?;
            session.status_message = format!("Recalculated {} formulas.", count);
        }
        MenuAction::TOCJump => {
            session.cursor = view_wb.compute_toc().pos;
        }
        MenuAction::BasicRun => session.status_message = "Choose a BASIC procedure to run.".into(),
    }
    Ok(false)
}

fn reload_workbook(path: &str, wb: &mut Workbook, active_sheet_idx: &mut usize, session: &mut lotus_core::WorkspaceSession) -> io::Result<()> {
            let bytes = fs::read(path)?;
            let loaded = deserialize_workbook(&bytes).map_err(|err| {
                io::Error::new(io::ErrorKind::InvalidData, format!("Invalid workbook: {:?}", err))
            })?;
            if loaded.sheets.is_empty() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Reloaded workbook contains no sheets"));
            }
            *wb = loaded;
            *active_sheet_idx = (*active_sheet_idx).min(wb.sheets.len() - 1);
            session.cursor = Coord::new(0, 0);
            session.is_dirty = false;
            session.status_message = "Workbook reloaded from disk.".into();
    Ok(())
}

fn run_demo() {
    println!("Creating sample Lotus 5-7-11 workbook: demo.lotus...");
    let mut wb = Workbook::new("demo-01", "Lotus 5-7-11 Demonstration");
    let mut sheet = Sheet::new("main", "Main Model");

    sheet.set_cell(Cell::number(Coord::new(0, 0), 8000.0).with_name("revenue"));
    sheet.set_cell(Cell::number(Coord::new(0, 1), 0.20).with_name("tax_rate"));
    // A3 depends on A1 (revenue) and A2 (tax_rate)
    sheet.set_cell(Cell::formula(Coord::new(0, 2), "=A1 * A2").with_name("tax_amount"));
    sheet.set_cell(Cell::number(Coord::new(1, 1), 0.0).with_name("tax_amount_script"));

    let basic_code = r#"SUB RunSimulation()
  LET rev = GET_CELL("revenue")
  LET tax = GET_CELL("tax_rate")
  SET_CELL("tax_amount_script", rev * tax)
END SUB"#;
    sheet.set_cell(Cell::basic(Coord::new(1, 0), "sim.bas", basic_code)
        .with_basic_grants(vec!["revenue".into(), "tax_rate".into()], vec!["tax_amount_script".into()]));

    let binary_data: Vec<u8> = (0..32).collect();
    sheet.set_cell(Cell::blob(Coord::new(2, 0), "weights.bin", binary_data));

    // Spatial extent cell at B26 (col 1, row 25)
    sheet.set_cell(Cell::text(Coord::new(1, 25), "Summary Marker"));

    wb.sheets.push(sheet);

    // Embed an executive briefing Markdown document referencing live spreadsheet cells
    let doc_body = r#"# Lotus 5-7-11 Executive Briefing

This self-contained computational workspace models top-line performance:
- Projected Revenue: **${{cell:revenue}}**
- Current Tax Rate: **{{cell:tax_rate}}**
- Calculated Tax: **${{cell:tax_amount}}**

*Note: This document updates live whenever cell values or formulas change.*"#;

    wb = wb.with_document(lotus_core::Document::new(
        "briefing",
        "Executive Briefing",
        doc_body,
    ));

    // Run dependency recalculation
    let evaluated_count = CalcEngine::recalculate_workbook(&mut wb).expect("Recalc failed");
    println!("Evaluated {} formulas across workbook.", evaluated_count);

    let bytes = serialize_workbook(&wb);
    fs::write("demo.lotus", &bytes).expect("Failed writing demo.lotus");
    println!("Successfully wrote demo.lotus ({} bytes).", bytes.len());

    let toc = wb.compute_toc();
    println!("Dynamic TOC calculated at: {} (extents X:{}, Y:{})", toc.pos, toc.extent_col, toc.extent_row);
}

fn recalc_file(path: &str) {
    let data = fs::read(path).expect("Failed reading file");
    let mut wb = deserialize_workbook(&data).expect("Failed parsing .lotus workbook");

    println!("Recalculating workbook '{}'...", path);
    let count = CalcEngine::recalculate_workbook(&mut wb).expect("Recalculation error");
    println!("Recalculated {} formula nodes successfully.", count);

    let updated_bytes = serialize_workbook(&wb);
    fs::write(path, updated_bytes).expect("Failed saving recalculated workbook");
    println!("Saved updated workbook to '{}'.", path);
}

fn run_basic_cell(file_path: &str, script_cell_name: &str, grant_args: &[String]) {
    let data = fs::read(file_path).expect("Failed reading file");
    let mut wb = deserialize_workbook(&data).expect("Failed parsing .lotus workbook");

    // Find script source
    let mut script: Option<(usize, Coord, String, Vec<String>, Vec<String>)> = None;

    for (s_idx, sheet) in wb.sheets.iter().enumerate() {
        for cell in sheet.cells.values() {
            if cell.name.as_deref() == Some(script_cell_name) {
                if let CellValue::Basic { source } = &cell.value {
                    script = Some((s_idx, cell.coord, source.clone(), cell.basic_reads.clone(), cell.basic_writes.clone()));
                    break;
                }
            }
        }
    }

    let (sheet_idx, owner, source, declared_reads, declared_writes) = match script {
        Some(s) => s,
        None => {
            eprintln!("Error: BASIC script cell '{}' not found.", script_cell_name);
            return;
        }
    };

    let mut all_args = vec!["--read".to_string(), declared_reads.join(","), "--write".to_string(), declared_writes.join(",")];
    all_args.extend_from_slice(grant_args);
    let (read_grants, write_grants) = match parse_basic_grants(&wb.sheets[sheet_idx], &all_args) {
        Ok(grants) => grants,
        Err(err) => {
            eprintln!("Invalid BASIC grants: {}", err);
            return;
        }
    };
    println!("Executing embedded BASIC script '{}' at {}...", script_cell_name, owner);

    match lotus_core::BasicRunner::execute(
        &mut wb.sheets[sheet_idx], owner, &source, &read_grants, &write_grants, Some(100_000),
    ) {
        Ok(writes) => {
            println!("Script completed successfully. {} staged writes committed.", writes);
            let updated_bytes = serialize_workbook(&wb);
            fs::write(file_path, updated_bytes).expect("Failed writing updated workbook");
            println!("Updated workbook saved to '{}'.", file_path);
        }
        Err(e) => {
            eprintln!("Execution error: {:?}", e);
        }
    }
}

fn parse_basic_grants(sheet: &Sheet, args: &[String]) -> Result<(Vec<Coord>, Vec<Coord>), String> {
    let mut readable = Vec::new();
    let mut writable = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let kind = args[i].as_str();
        let targets = args.get(i + 1).ok_or_else(|| format!("missing targets after {}", kind))?;
        let grant_list = match kind {
            "--read" => &mut readable,
            "--write" => &mut writable,
            _ => return Err(format!("unknown option '{}'; expected --read or --write", kind)),
        };
        for target in targets.split(',').filter(|part| !part.is_empty()) {
            let coord = Coord::parse_a1(target).or_else(|| {
                sheet.cells.values().find(|cell| cell.name.as_deref() == Some(target)).map(|cell| cell.coord)
            }).ok_or_else(|| format!("'{}' is not a cell coordinate or name on this sheet", target))?;
            grant_list.push(coord);
        }
        i += 2;
    }
    Ok((readable, writable))
}

fn declared_basic_grants(sheet: &Sheet, reads: &[String], writes: &[String]) -> Result<(Vec<Coord>, Vec<Coord>), String> {
    let args = vec!["--read".into(), reads.join(","), "--write".into(), writes.join(",")];
    parse_basic_grants(sheet, &args)
}

fn inspect_file(path: &str) {
    let data = fs::read(path).expect("Failed reading file");
    let wb = deserialize_workbook(&data).expect("Failed parsing .lotus workbook");

    println!("============================================================");
    println!("LOTUS 5-7-11 WORKBOOK: {} (\"{}\")", wb.id, wb.title);
    println!("Calculation Mode: {}", wb.calc_mode);

    let toc = wb.compute_toc();
    println!("Dynamic TOC: {} [maxX: {}, maxY: {}]", toc.pos, toc.extent_col, toc.extent_row);
    println!("TOC Entries ({} registered items):", toc.entries.len());
    for entry in &toc.entries {
        println!("  - [{}] {:<12} in sheet '{}' ({}, {} bytes)", entry.coord, entry.name, entry.sheet_id, entry.type_name, entry.byte_size);
    }

    for sheet in &wb.sheets {
        println!("------------------------------------------------------------");
        println!("Sheet: {} (\"{}\") — {} cells", sheet.id, sheet.name, sheet.cells.len());
        for cell in sheet.cells.values() {
            let name_str = cell.name.as_deref().map(|n| format!(" ({})", n)).unwrap_or_default();
            match &cell.value {
                CellValue::Number(n) => println!("  {:<6}{} : Number = {}", cell.coord, name_str, n),
                CellValue::Text(t) => println!("  {:<6}{} : Text = \"{}\"", cell.coord, name_str, t),
                CellValue::Boolean(b) => println!("  {:<6}{} : Boolean = {}", cell.coord, name_str, b),
                CellValue::Formula { expr, cached_num, cached_text, cached_bool } => {
                    let cached = cached_num.map(|n| n.to_string())
                        .or_else(|| cached_bool.map(|value| if value { "TRUE".into() } else { "FALSE".into() }))
                        .or_else(|| cached_text.clone()).unwrap_or_else(|| "<pending>".to_string());
                    println!("  {:<6}{} : Formula {} => {}", cell.coord, name_str, expr, cached);
                }
                CellValue::Basic { source } => println!("  {:<6}{} : BASIC Script ({} bytes)", cell.coord, name_str, source.len()),
                CellValue::Blob { bytes } => println!("  {:<6}{} : Raw Binary Payload ({} bytes)", cell.coord, name_str, bytes.len()),
                CellValue::Empty => println!("  {:<6}{} : Empty", cell.coord, name_str),
                CellValue::Error(e) => println!("  {:<6}{} : Error = {}", cell.coord, name_str, e),
            }
        }
    }
    println!("============================================================");
}

fn inspect_history(path: &str, requested_sheet: Option<&str>) {
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(err) => {
            eprintln!("Unable to read '{}': {}", path, err);
            return;
        }
    };
    let wb = match deserialize_workbook(&data) {
        Ok(wb) => wb,
        Err(err) => {
            eprintln!("Invalid workbook: {:?}", err);
            return;
        }
    };

    let mut found = false;
    for sheet in &wb.sheets {
        if requested_sheet.is_some_and(|id| id != sheet.id) {
            continue;
        }
        if let Some(history) = wb.history_for_sheet(&sheet.id) {
            found = true;
            println!("Sheet '{}' — Z=0 is the base; latest layer is Z={}", sheet.id, history.max_z);
            for (z, records) in &history.layers {
                println!("  Z={}", z);
                for record in records {
                    match &record.cell {
                        Some(cell) => println!("    {} <- {}", record.coord, describe_history_cell(cell)),
                        None => println!("    {} <- deleted", record.coord),
                    }
                }
            }
        }
    }
    if requested_sheet.is_some_and(|id| wb.get_sheet(id).is_none()) {
        eprintln!("Sheet '{}' not found.", requested_sheet.unwrap());
    } else if !found {
        println!("No Z-layer updates stored in this workbook.");
    }
}

fn describe_history_cell(cell: &Cell) -> String {
    match &cell.value {
        CellValue::Empty => "empty".into(),
        CellValue::Number(value) => format!("number {}", value),
        CellValue::Text(value) => format!("text {:?}", value),
        CellValue::Boolean(value) => format!("boolean {}", value),
        CellValue::Formula { expr, .. } => format!("formula {}", expr),
        CellValue::Basic { source } => format!("BASIC ({} bytes)", source.len()),
        CellValue::Blob { bytes } => format!("blob ({} bytes)", bytes.len()),
        CellValue::Error(value) => format!("error {}", value),
    }
}

fn view_history_layer(path: &str, sheet_id: &str, z_arg: &str) {
    let z = match z_arg.parse::<u32>() {
        Ok(z) => z,
        Err(_) => {
            eprintln!("Invalid Z layer '{}'.", z_arg);
            return;
        }
    };
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(err) => {
            eprintln!("Unable to read '{}': {}", path, err);
            return;
        }
    };
    let wb = match deserialize_workbook(&data) {
        Ok(wb) => wb,
        Err(err) => {
            eprintln!("Invalid workbook: {:?}", err);
            return;
        }
    };
    let mut sheet = match wb.sheet_at_z(sheet_id, z) {
        Some(sheet) => sheet,
        None => {
            eprintln!("Sheet '{}' or Z layer {} not found.", sheet_id, z);
            return;
        }
    };
    if let Err(err) = CalcEngine::recalculate_sheet(&mut sheet, None) {
        eprintln!("Recalculation warning: {:?}", err);
    }
    println!("LOTUS 5-7-11: sheet '{}' at Z={} (base is Z=0)", sheet_id, z);
    print!("{}", lotus_core::GridRenderer::render_sheet(&sheet, None, None, None));
}

fn view_sheet(path: &str) {
    let data = fs::read(path).expect("Failed reading file");
    let wb = deserialize_workbook(&data).expect("Failed parsing .lotus workbook");

    println!("LOTUS 5-7-11 WORKSPACE: {} (\"{}\")", wb.id, wb.title);
    let toc = wb.compute_toc();
    println!("Dynamic TOC Position: {} [Extents X:{}, Y:{}]\n", toc.pos, toc.extent_col, toc.extent_row);

    if let Some(sheet) = wb.primary_sheet() {
        let rendered = lotus_core::GridRenderer::render_sheet(sheet, Some(&toc), None, None);
        print!("{}", rendered);
    }
}

fn render_screen_cli(path: &str) {
    let data = fs::read(path).expect("Failed reading file");
    let wb = deserialize_workbook(&data).expect("Failed parsing .lotus workbook");
    let toc = wb.compute_toc();

    if let Some(sheet) = wb.primary_sheet() {
        let mut session = lotus_core::WorkspaceSession::new();
        // Demonstrate opening Lotus slash menu
        session.menu.open();
        let screen = session.render_workspace(sheet, &toc);
        print!("{}", screen);
    }
}

fn view_document_cli(path: &str, target_id: Option<&str>) {
    let data = fs::read(path).expect("Failed reading file");
    let wb = deserialize_workbook(&data).expect("Failed parsing .lotus workbook");

    if wb.documents.is_empty() {
        println!("No embedded documents in this workbook.");
        return;
    }

    match target_id {
        Some(id) => match wb.get_document(id) {
            Some(doc) => {
                println!("============================================================");
                println!("DOCUMENT: {} (\"{}\") [{}]", doc.id, doc.title, doc.content_type);
                println!("============================================================");
                let rendered = if let Some(sheet) = wb.primary_sheet() {
                    doc.render_live(sheet, Some(&wb))
                } else {
                    doc.body.clone()
                };
                println!("{}", rendered);
                println!("============================================================");
            }
            None => eprintln!("Error: Document '{}' not found in workbook.", id),
        },
        None => {
            println!("Embedded Documents ({} registered):", wb.documents.len());
            for doc in &wb.documents {
                println!("  - [{}] \"{}\" ({})", doc.id, doc.title, doc.content_type);
            }
            println!("\nUse 'lotus doc <file> <id>' to render a specific document with live cell references.");
        }
    }
}

fn set_cell_cli(file_path: &str, target: &str, raw_val: &str) {
    let data = fs::read(file_path).expect("Failed reading file");
    let mut wb = deserialize_workbook(&data).expect("Failed parsing .lotus workbook");

    let sheet = match wb.primary_sheet_mut() {
        Some(s) => s,
        None => {
            eprintln!("Error: Workbook contains no sheets.");
            return;
        }
    };

    // Find coordinate by A1 parse or by cell name
    let coord = if let Some(c) = Coord::parse_a1(target) {
        c
    } else {
        let mut found_c = None;
        for cell in sheet.cells.values() {
            if cell.name.as_deref() == Some(target) {
                found_c = Some(cell.coord);
                break;
            }
        }
        match found_c {
            Some(c) => c,
            None => {
                eprintln!("Error: Target '{}' is neither a valid coordinate nor a known cell name.", target);
                return;
            }
        }
    };

    let cell_value = if raw_val.starts_with('=') {
        CellValue::Formula {
            expr: raw_val.to_string(),
            cached_num: None,
            cached_text: None,
            cached_bool: None,
        }
    } else if let Ok(n) = raw_val.parse::<f64>() {
        CellValue::Number(n)
    } else if raw_val.eq_ignore_ascii_case("true") {
        CellValue::Boolean(true)
    } else if raw_val.eq_ignore_ascii_case("false") {
        CellValue::Boolean(false)
    } else {
        CellValue::Text(raw_val.to_string())
    };

    if let Some(cell) = sheet.cells.get_mut(&coord) {
        cell.value = cell_value;
    } else {
        sheet.set_cell(Cell::new(coord, cell_value));
    }

    println!("Cell '{}' set.", target);
    let count = CalcEngine::recalculate_workbook(&mut wb).expect("Recalc error");
    println!("Recalculated {} formulas.", count);

    let updated = serialize_workbook(&wb);
    fs::write(file_path, updated).expect("Failed saving workbook");
    println!("Saved changes to '{}'.", file_path);
}

fn import_cell_cli(file_path: &str, coord_str: &str, name: &str, import_path: &str) {
    let coord = match Coord::parse_a1(coord_str) {
        Some(c) => c,
        None => {
            eprintln!("Error: Invalid coordinate '{}'", coord_str);
            return;
        }
    };

    let payload_bytes = fs::read(import_path).expect("Failed reading file to import");
    let data = fs::read(file_path).expect("Failed reading workbook");
    let mut wb = deserialize_workbook(&data).expect("Failed parsing .lotus workbook");

    wb.import_blob(0, coord, name.to_string(), payload_bytes.clone()).expect("Import error");
    println!("Imported '{}' ({} bytes) into cell {} as '{}'.", import_path, payload_bytes.len(), coord, name);

    let updated = serialize_workbook(&wb);
    fs::write(file_path, updated).expect("Failed saving workbook");
    println!("Saved changes to '{}'.", file_path);
}

fn eject_named_cell(file_path: &str, cell_name: &str, out_path: &str) {
    let data = fs::read(file_path).expect("Failed reading file");
    let wb = deserialize_workbook(&data).expect("Failed parsing .lotus workbook");

    match wb.eject_cell(cell_name) {
        Some(bytes) => {
            fs::write(out_path, bytes).expect("Failed writing ejected file");
            println!("Successfully ejected cell '{}' to '{}' ({} bytes).", cell_name, out_path, bytes.len());
        }
        None => {
            eprintln!("Error: Named cell '{}' not found in workbook.", cell_name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{append_external_layers, collect_basic_choices, compose_current_workbook, declared_basic_grants, file_hash, file_poll_interval, newer_history_z, next_sheet_index, older_history_z, poll_external_workbook, sheet_changes, terminal_line_endings, viewport_dimensions};
    use lotus_core::{Cell, CellValue, Coord, Sheet, Workbook};
    use std::time::{Duration, Instant};

    #[test]
    fn terminal_resize_updates_viewport_dimensions() {
        assert_eq!(viewport_dimensions(80, 24), (4, 16));
        assert_eq!(viewport_dimensions(120, 40), (7, 32));
        assert_eq!(viewport_dimensions(1, 1), (1, 1));
    }

    #[test]
    fn file_polling_is_slow_by_default_and_bursts_after_a_user_save() {
        let now = Instant::now();
        assert_eq!(file_poll_interval(None, now), Duration::from_secs(30));
        assert_eq!(file_poll_interval(Some(now + Duration::from_secs(60)), now), Duration::from_secs(10));
        assert_eq!(file_poll_interval(Some(now), now), Duration::from_secs(30));
    }

    #[test]
    fn raw_terminal_output_uses_carriage_return_line_feed() {
        assert_eq!(terminal_line_endings("header\nrow\n"), "header\r\nrow\r\n");
    }

    #[test]
    fn sheet_switching_wraps_and_basic_choices_are_scoped_to_the_sheet() {
        assert_eq!(next_sheet_index(0, 3, -1), 2);
        assert_eq!(next_sheet_index(2, 3, 1), 0);
        assert_eq!(next_sheet_index(0, 1, 1), 0);

        let mut sheet = Sheet::new("main", "Main");
        sheet.set_cell(Cell::basic(Coord::new(0, 0), "first.bas", "SUB First()\nEND SUB"));
        sheet.set_cell(Cell::text(Coord::new(1, 0), "not a script"));
        sheet.set_cell(Cell::basic(Coord::new(2, 0), "second.bas", "SUB Second()\nEND SUB"));

        let choices = collect_basic_choices(&sheet);
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].name, "first.bas");
        assert_eq!(choices[1].coord, Coord::new(2, 0));
    }

    #[test]
    fn history_browsing_steps_toward_base_and_returns_to_live_at_latest() {
        assert_eq!(older_history_z(2), 1);
        assert_eq!(older_history_z(1), 0);
        assert_eq!(older_history_z(0), 0);
        assert_eq!(newer_history_z(0, 3), Some(1));
        assert_eq!(newer_history_z(1, 3), Some(2));
        assert_eq!(newer_history_z(2, 3), None);
        assert_eq!(newer_history_z(u32::MAX, u32::MAX), None);
    }

    #[test]
    fn external_layer_batches_append_after_local_layers_without_changing_base() {
        let mut base = Sheet::new("main", "Main");
        base.set_cell(Cell::number(Coord::new(0, 0), 10.0));
        let mut current = Workbook::new("poll-wb", "Polling").with_sheet(base.clone());
        current.record_history_layer("main", vec![(Coord::new(0, 0), Some(Cell::number(Coord::new(0, 0), 20.0))) ]).unwrap();

        let mut incoming = Workbook::new("poll-wb", "Polling").with_sheet(base);
        incoming.record_history_layer("main", vec![(Coord::new(0, 0), Some(Cell::number(Coord::new(0, 0), 30.0))) ]).unwrap();
        incoming.record_history_layer("main", vec![(Coord::new(1, 0), Some(Cell::text(Coord::new(1, 0), "reply"))) ]).unwrap();

        assert_eq!(append_external_layers(&mut current, &incoming).unwrap(), 2);
        assert_eq!(current.sheets[0].get_cell(Coord::new(0, 0)).unwrap().value, CellValue::Number(10.0));
        assert_eq!(current.sheet_at_z("main", 1).unwrap().get_cell(Coord::new(0, 0)).unwrap().value, CellValue::Number(20.0));
        assert_eq!(current.sheet_at_z("main", 2).unwrap().get_cell(Coord::new(0, 0)).unwrap().value, CellValue::Number(30.0));
        assert_eq!(current.history_for_sheet("main").unwrap().max_z, 3);
    }

    #[test]
    fn file_poll_imports_appended_update_and_rejects_base_rewrites() {
        let base = Workbook::new("poll-wb", "Polling").with_sheet(Sheet::new("main", "Main"));
        let original_bytes = lotus_core::serialize_workbook(&base);
        let path = std::env::temp_dir().join(format!("lotus-poll-{}.lotus", std::process::id()));
        std::fs::write(&path, &original_bytes).unwrap();
        let mut current = base;
        let mut known_hash = file_hash(&original_bytes);
        let mut observed_hash = known_hash;

        let mut updated_bytes = original_bytes.clone();
        updated_bytes.extend_from_slice(b"\n<history><layer sheet=\"main\" z=\"1\"><cell ref=\"A1\" type=\"number\">42</cell></layer><layer sheet=\"main\" z=\"2\"><cell ref=\"B1\" type=\"text\">reply</cell></layer></history>\n");
        std::fs::write(&path, &updated_bytes).unwrap();
        let message = poll_external_workbook(path.to_str().unwrap(), &mut current, &mut known_hash, &mut observed_hash).unwrap();
        assert_eq!(message.as_deref(), Some("Received 2 new Z layers from the workbook file."));
        assert_eq!(current.history_for_sheet("main").unwrap().max_z, 2);

        let mut changed_base = current.clone();
        changed_base.sheets[0].set_cell(Cell::text(Coord::new(2, 0), "not a move"));
        std::fs::write(&path, lotus_core::serialize_workbook(&changed_base)).unwrap();
        let message = poll_external_workbook(path.to_str().unwrap(), &mut current, &mut known_hash, &mut observed_hash).unwrap();
        assert!(message.unwrap().contains("only append-only Z-layer updates"));
        assert!(current.sheets[0].get_cell(Coord::new(2, 0)).is_none());
        assert_ne!(known_hash, observed_hash, "rejected input must block a later save");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn workspace_basic_runs_with_grants_declared_on_its_cell() {
        let mut sheet = Sheet::new("main", "Main");
        sheet.set_cell(Cell::number(Coord::new(0, 0), 9.0).with_name("input"));
        sheet.set_cell(Cell::number(Coord::new(1, 0), 0.0).with_name("output"));
        sheet.set_cell(Cell::basic(Coord::new(2, 0), "script", "LET v = GET_CELL(\"input\")\nSET_CELL(\"output\", v + 1)")
            .with_basic_grants(vec!["input".into()], vec!["output".into()]));
        let choice = collect_basic_choices(&sheet).remove(0);
        let (reads, writes) = declared_basic_grants(&sheet, &choice.reads, &choice.writes).unwrap();
        lotus_core::BasicRunner::execute(&mut sheet, choice.coord, &choice.source, &reads, &writes, None).unwrap();
        assert_eq!(sheet.get_cell(Coord::new(1, 0)).unwrap().value, CellValue::Number(10.0));
    }

    #[test]
    fn proposed_patch_tracks_edits_and_deletions_but_skips_formula_cache_changes() {
        let mut before = Sheet::new("main", "Main");
        let mut after = Sheet::new("main", "Main");
        before.set_cell(Cell::new(Coord::new(0, 0), CellValue::Formula {
            expr: "=B1*2".into(), cached_num: Some(2.0), cached_text: None, cached_bool: None,
        }));
        after.set_cell(Cell::new(Coord::new(0, 0), CellValue::Formula {
            expr: "=B1*2".into(), cached_num: Some(8.0), cached_text: None, cached_bool: None,
        }));
        before.set_cell(Cell::number(Coord::new(1, 0), 1.0));
        after.set_cell(Cell::number(Coord::new(1, 0), 4.0));
        before.set_cell(Cell::text(Coord::new(2, 0), "remove me"));

        let changes = sheet_changes(&before, &after);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0], (Coord::new(1, 0), Some(Cell::number(Coord::new(1, 0), 4.0))));
        assert_eq!(changes[1], (Coord::new(2, 0), None));
    }

    #[test]
    fn editor_patch_composes_as_latest_layer_without_changing_base() {
        let mut sheet = Sheet::new("main", "Main");
        sheet.set_cell(Cell::number(Coord::new(0, 0), 10.0));
        let mut wb = Workbook::new("patch-wb", "Patch Workspace").with_sheet(sheet);

        let mut view = compose_current_workbook(&wb);
        view.sheets[0].set_cell(Cell::number(Coord::new(1, 0), 20.0));
        let changes = sheet_changes(&wb.sheets[0], &view.sheets[0]);
        assert_eq!(wb.record_history_layer("main", changes).unwrap(), 1);

        let latest = compose_current_workbook(&wb);
        assert_eq!(wb.sheets[0].cells.len(), 1);
        assert_eq!(latest.sheets[0].get_cell(Coord::new(1, 0)).unwrap().value, CellValue::Number(20.0));
    }
}
