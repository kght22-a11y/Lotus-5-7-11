use lotus_core::{
    deserialize_workbook, serialize_workbook, CalcEngine, Cell, CellValue, Coord, Sheet, Workbook,
};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::env;
use std::fs;
use std::io::{self, Write};

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
    let mut wb = deserialize_workbook(&data)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid workbook: {:?}", err)))?;
    if wb.sheets.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Workbook contains no sheets"));
    }

    let mut session = lotus_core::WorkspaceSession::new();
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    if let Err(err) = execute!(stdout, EnterAlternateScreen, Hide) {
        let _ = terminal::disable_raw_mode();
        return Err(err);
    }

    let result = (|| -> io::Result<()> {
        loop {
            let toc = wb.compute_toc();
            let (terminal_width, terminal_height) = terminal::size().unwrap_or((80, 24));
            let (viewport_cols, viewport_rows) = viewport_dimensions(terminal_width, terminal_height);
            let sheet = wb.primary_sheet().expect("sheet presence checked");
            execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
            write!(stdout, "{}", session.render_workspace_with_viewport(
                sheet,
                &toc,
                viewport_cols,
                viewport_rows,
            ))?;
            writeln!(stdout, "\nKeys: arrows move | Enter edits | / menu | Ctrl-S saves | q quits")?;
            stdout.flush()?;

            let key = match event::read()? {
                Event::Resize(_, _) => continue,
                Event::Key(key) if key.kind == crossterm::event::KeyEventKind::Press => key,
                _ => continue,
            };

            if session.edit_buffer.is_some() {
                handle_edit_key(&mut session, &mut wb.sheets[0], key);
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
                if handle_menu_action(action, path, &mut wb, &mut session)? {
                    break;
                }
                continue;
            }

            match key.code {
                KeyCode::Char('/') => session.menu.open(),
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    save_workbook(path, &wb)?;
                    session.status_message = format!("Saved {}", path);
                }
                KeyCode::Enter => session.start_editing(wb.primary_sheet().unwrap()),
                KeyCode::Left => move_workspace_cursor(&mut session, -1, 0),
                KeyCode::Right => move_workspace_cursor(&mut session, 1, 0),
                KeyCode::Up => move_workspace_cursor(&mut session, 0, -1),
                KeyCode::Down => move_workspace_cursor(&mut session, 0, 1),
                _ => {}
            }
        }
        Ok(())
    })();

    let screen_restore = execute!(stdout, Show, LeaveAlternateScreen);
    let raw_mode_restore = terminal::disable_raw_mode();
    result.and(screen_restore).and(raw_mode_restore)
}

fn viewport_dimensions(terminal_width: u16, terminal_height: u16) -> (u32, u32) {
    let cols = (terminal_width.saturating_sub(10) / 15).max(1).min(26) as u32;
    let rows = terminal_height.saturating_sub(6).max(1) as u32;
    (cols, rows)
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

fn save_workbook(path: &str, wb: &Workbook) -> io::Result<()> {
    fs::write(path, serialize_workbook(wb))
}

fn handle_menu_action(
    action: lotus_core::MenuAction,
    path: &str,
    wb: &mut Workbook,
    session: &mut lotus_core::WorkspaceSession,
) -> io::Result<bool> {
    use lotus_core::MenuAction;
    match action {
        MenuAction::None => {}
        MenuAction::Quit => return Ok(true),
        MenuAction::FileSave => {
            save_workbook(path, wb)?;
            session.status_message = format!("Saved {}", path);
        }
        MenuAction::FileReload => {
            let bytes = fs::read(path)?;
            let loaded = deserialize_workbook(&bytes).map_err(|err| {
                io::Error::new(io::ErrorKind::InvalidData, format!("Invalid workbook: {:?}", err))
            })?;
            if loaded.sheets.is_empty() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Reloaded workbook contains no sheets"));
            }
            *wb = loaded;
            session.cursor = Coord::new(0, 0);
            session.status_message = "Workbook reloaded from disk.".into();
        }
        MenuAction::FileInspect => {
            let toc = wb.compute_toc();
            session.status_message = format!("{} sheets, {} named entries; TOC at {}", wb.sheets.len(), toc.entries.len(), toc.pos);
        }
        MenuAction::RecalculateAll => {
            let count = CalcEngine::recalculate_workbook(wb).map_err(|err| {
                io::Error::new(io::ErrorKind::Other, format!("Recalculation failed: {:?}", err))
            })?;
            session.status_message = format!("Recalculated {} formulas.", count);
        }
        MenuAction::TOCJump => {
            session.cursor = wb.compute_toc().pos;
        }
        MenuAction::BasicRun => {
            let script = wb.sheets.iter().enumerate().find_map(|(idx, sheet)| {
                sheet.cells.values().find_map(|cell| match &cell.value {
                    CellValue::Basic { source } => Some((idx, cell.coord, source.clone())),
                    _ => None,
                })
            });
            let result = script.map(|(idx, owner, source)| {
                lotus_core::BasicRunner::execute(&mut wb.sheets[idx], owner, &source, &[], &[], Some(100_000))
            });
            session.status_message = match result {
                Some(Ok(count)) => format!("BASIC completed: {} writes.", count),
                Some(Err(err)) => format!("BASIC error: {:?}", err),
                None => "No BASIC procedure found in workbook.".into(),
            };
        }
    }
    Ok(false)
}

fn run_demo() {
    println!("Creating sample Lotus 5-7-11 workbook: demo.lotus...");
    let mut wb = Workbook::new("demo-01", "Lotus 5-7-11 Demonstration");
    let mut sheet = Sheet::new("main", "Main Model");

    sheet.set_cell(Cell::number(Coord::new(0, 0), 1500.0).with_name("revenue"));
    sheet.set_cell(Cell::number(Coord::new(0, 1), 0.20).with_name("tax_rate"));
    // A3 depends on A1 (revenue) and A2 (tax_rate)
    sheet.set_cell(Cell::formula(Coord::new(0, 2), "=A1 * A2").with_name("tax_amount"));

    let basic_code = r#"SUB RunSimulation()
  LET rev = GET_CELL("revenue")
  LET tax = GET_CELL("tax_rate")
  SET_CELL("tax_amount", rev * tax)
END SUB"#;
    sheet.set_cell(Cell::basic(Coord::new(1, 0), "sim.bas", basic_code));

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
    let mut script: Option<(usize, Coord, String)> = None;

    for (s_idx, sheet) in wb.sheets.iter().enumerate() {
        for cell in sheet.cells.values() {
            if cell.name.as_deref() == Some(script_cell_name) {
                if let CellValue::Basic { source } = &cell.value {
                    script = Some((s_idx, cell.coord, source.clone()));
                    break;
                }
            }
        }
    }

    let (sheet_idx, owner, source) = match script {
        Some(s) => s,
        None => {
            eprintln!("Error: BASIC script cell '{}' not found.", script_cell_name);
            return;
        }
    };

    let (read_grants, write_grants) = match parse_basic_grants(&wb.sheets[sheet_idx], grant_args) {
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
                CellValue::Formula { expr, cached_num, cached_text } => {
                    let cached = cached_num.map(|n| n.to_string()).or_else(|| cached_text.clone()).unwrap_or_else(|| "<pending>".to_string());
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
    use super::viewport_dimensions;

    #[test]
    fn terminal_resize_updates_viewport_dimensions() {
        assert_eq!(viewport_dimensions(80, 24), (4, 18));
        assert_eq!(viewport_dimensions(120, 40), (7, 34));
        assert_eq!(viewport_dimensions(1, 1), (1, 1));
    }
}
