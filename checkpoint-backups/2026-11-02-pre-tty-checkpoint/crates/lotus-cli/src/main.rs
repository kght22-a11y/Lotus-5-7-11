use lotus_core::{
    deserialize_workbook, serialize_workbook, CalcEngine, Cell, CellValue, Coord, Sheet, Workbook,
};
use std::env;
use std::fs;

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
                eprintln!("Usage: lotus run-basic <file.lotus> <script_cell_name>");
                return;
            }
            run_basic_cell(&args[2], &args[3]);
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
    println!("  lotus doc <file.lotus> [id]                  View embedded live Markdown documents");
    println!("  lotus inspect <file.lotus>                   Inspect workbook structure & TOC");
    println!("  lotus set <file.lotus> <coord/name> <val>    Set cell value or formula and recalc");
    println!("  lotus recalc <file.lotus>                    Recalculate formulas and update file");
    println!("  lotus run-basic <file.lotus> <cell_name>     Execute embedded BASIC script");
    println!("  lotus eject <file.lotus> <name> <out>        Eject cell payload to file");
    println!("  lotus import <file.lotus> <coord> <name> <f> Import file into named cell payload");
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

fn run_basic_cell(file_path: &str, script_cell_name: &str) {
    let data = fs::read(file_path).expect("Failed reading file");
    let mut wb = deserialize_workbook(&data).expect("Failed parsing .lotus workbook");

    // Find script source
    let mut script_source: Option<String> = None;
    let mut target_sheet_idx: Option<usize> = None;

    for (s_idx, sheet) in wb.sheets.iter().enumerate() {
        for cell in sheet.cells.values() {
            if cell.name.as_deref() == Some(script_cell_name) {
                if let CellValue::Basic { source } = &cell.value {
                    script_source = Some(source.clone());
                    target_sheet_idx = Some(s_idx);
                    break;
                }
            }
        }
    }

    let source = match script_source {
        Some(s) => s,
        None => {
            eprintln!("Error: BASIC script cell '{}' not found.", script_cell_name);
            return;
        }
    };

    let sheet_idx = target_sheet_idx.unwrap();
    println!("Executing embedded BASIC script '{}'...", script_cell_name);

    match lotus_core::BasicRunner::execute(&mut wb.sheets[sheet_idx], &source, None, Some(100_000)) {
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
