use lotus_core::*;

fn rt(wb: &Workbook) -> Result<Workbook, String> {
    let bytes = serialize_workbook(wb);
    deserialize_workbook(&bytes).map_err(|e| format!("{:?}", e))
}

fn sheet_with(cells: Vec<Cell>) -> Workbook {
    let mut s = Sheet::new("main", "Main");
    for c in cells { s.set_cell(c); }
    Workbook::new("t", "t").with_sheet(s)
}

fn report(label: &str, original: &Workbook) {
    match rt(original) {
        Err(e) => println!("[FAIL-PARSE] {label}: {e}"),
        Ok(back) if &back == original => println!("[ok]         {label}"),
        Ok(back) => {
            println!("[MISMATCH]   {label}");
            if let (Some(a), Some(b)) = (original.primary_sheet(), back.primary_sheet()) {
                for (k, c) in &a.cells {
                    match b.cells.get(k) {
                        Some(c2) if c2 == c => {}
                        Some(c2) => println!("    {:?}\n      wrote: {:?}\n      read : {:?}", k, c.value, c2.value),
                        None => println!("    {:?} lost: {:?}", k, c.value),
                    }
                }
            }
            if original.documents != back.documents {
                println!("    documents differ:\n      wrote: {:?}\n      read : {:?}", original.documents, back.documents);
            }
        }
    }
}

#[test]
fn edge_cases() {
    let c = |col, row| Coord::new(col, row);

    report("BASIC source containing ]]>",
        &sheet_with(vec![Cell::basic(c(0,0), "s.bas", "SUB A()\n  LET x = \"]]>\"\nEND SUB")]));

    let doc_wb = Workbook::new("t","t").with_document(Document::new("d","d","before ]]> after"));
    report("Document body containing ]]>", &doc_wb);

    report("Text with leading/trailing space",   &sheet_with(vec![Cell::text(c(0,0), "  padded  ")]));
    report("Text with newlines",                  &sheet_with(vec![Cell::text(c(0,0), "line1\nline2\n")]));
    report("Text with CRLF",                      &sheet_with(vec![Cell::text(c(0,0), "a\r\nb")]));
    report("Text with & < > and unicode",         &sheet_with(vec![Cell::text(c(0,0), "R&D <5 > 3 — café 日本")]));
    report("Text that looks like entity",         &sheet_with(vec![Cell::text(c(0,0), "&amp; literally")]));
    report("Empty text",                          &sheet_with(vec![Cell::text(c(0,0), "")]));

    for (l, n) in [("NaN", f64::NAN), ("+inf", f64::INFINITY), ("-inf", f64::NEG_INFINITY),
                   ("0.1+0.2", 0.1+0.2), ("1e300", 1e300), ("-0.0", -0.0), ("1/3", 1.0/3.0), ("5e-324", 5e-324)] {
        // NaN != NaN so compare via bits
        let wb = sheet_with(vec![Cell::number(c(0,0), n)]);
        match rt(&wb) {
            Err(e) => println!("[FAIL-PARSE] number {l}: {e}"),
            Ok(b) => {
                let got = b.primary_sheet().unwrap().get_cell(c(0,0)).map(|x| x.value.clone());
                let same = matches!(&got, Some(CellValue::Number(m)) if m.to_bits()==n.to_bits());
                println!("[{}] number {l}: got {:?}", if same {"ok"} else {"MISMATCH"}, got);
            }
        }
    }

    report("Blob with </cell> + NUL + invalid UTF-8",
        &sheet_with(vec![Cell::blob(c(0,0), "b.bin", b"x</cell>\0\xff\xfe<cell ref=\"Z9\" type=\"number\">1</cell>".to_vec())]));
    report("Empty blob", &sheet_with(vec![Cell::blob(c(0,0), "e.bin", vec![])]));

    report("Formula with quotes", &sheet_with(vec![Cell::formula(c(0,0), "=IF(1>0,\"a\",\"b\")")]));
    report("Name with quote/newline", &sheet_with(vec![Cell::number(c(0,0), 1.0).with_name("we\"ird\nname")]));
    report("Workbook title with newline", &{ let mut w = sheet_with(vec![]); w.title = "a\nb\tc".into(); w });
    report("Boolean", &sheet_with(vec![Cell::boolean(c(0,0), true)]));
    report("Error cell", &sheet_with(vec![Cell::new(c(0,0), CellValue::Error("#DIV/0!".into()))]));
    report("Empty cell", &sheet_with(vec![Cell::new(c(0,0), CellValue::Empty)]));

    // Large coordinates (sparse) — TOC position / A1 conversion
    let big = sheet_with(vec![Cell::number(c(16383, 1048575), 1.0)]);
    report("Cell at XFD1048576", &big);
    println!("    TOC at: {}", big.compute_toc().pos.to_a1());

    // Duplicate names
    let dup = sheet_with(vec![Cell::text(c(0,0), "first").with_name("x"), Cell::text(c(1,0), "second").with_name("x")]);
    println!("[info] duplicate name 'x' eject -> {:?}", dup.eject_cell("x").map(|b| String::from_utf8_lossy(b).to_string()));
}

#[test]
fn literal_entities_in_text_and_names() {
    let c = |col, row| Coord::new(col, row);
    report("Text that is literally '&lt;'", &sheet_with(vec![Cell::text(c(0,0), "&lt;b&gt; &quot;x&quot; &amp;amp;")]));
    report("Cached formula text with & <", &sheet_with(vec![Cell::new(c(0,0), CellValue::Formula{expr:"=A2".into(), cached_num:None, cached_text:Some("R&D <5".into())})]));
    report("Workbook title with quote & amp", &{ let mut w = sheet_with(vec![]); w.title = "Q&A \"2026\"".into(); w });
}
