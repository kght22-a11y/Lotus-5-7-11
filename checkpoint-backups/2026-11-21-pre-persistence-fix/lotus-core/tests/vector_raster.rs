use lotus_core::{
    deserialize_workbook, serialize_workbook, Bitmap, Cell, CellValue, ColorRgba, Coord,
    HersheyFont, Sheet, SubpixelRasterizer, Transform2D, VectorBuffer, Workbook,
};

#[test]
fn test_hershey_text_bitmap_rasterization_and_bmp_export() {
    let font = HersheyFont::load_simplex();
    let mut vec_buf = VectorBuffer::new();

    vec_buf.set_color(ColorRgba::rgb(255, 255, 255));
    vec_buf.set_width(1.0);

    let text = "Lotus 5-7-11";
    let advance = font.draw_string(text, 20.0, 30.0, 1.0, &mut vec_buf);
    assert!(advance > 50.0);

    let mut bitmap = Bitmap::new(200, 60);
    bitmap.clear(0x00000000); // Black background

    SubpixelRasterizer::render(&mut bitmap, &vec_buf, &Transform2D::identity());

    // Count non-black pixels
    let lit_pixels = bitmap.pixels.iter().filter(|&&p| p != 0).count();
    assert!(
        lit_pixels > 100,
        "Rendered text should illuminate more than 100 pixels, got {}",
        lit_pixels
    );

    // Export to BMP
    let bmp_bytes = bitmap.to_bmp_bytes();
    assert_eq!(&bmp_bytes[0..2], b"BM");
    assert!(bmp_bytes.len() >= 54 + (200 * 3 * 60));
}

#[test]
fn test_far_out_vector_asset_cells_do_not_distort_toc() {
    let mut wb = Workbook::new("test_wb", "Test Workbook");
    let mut sheet = Sheet::new("main", "Main Sheet");

    // Normal active spreadsheet data in A1:C26
    sheet.set_cell(Cell::number(Coord::new(0, 0), 100.0).with_name("revenue")); // A1
    sheet.set_cell(Cell::number(Coord::new(1, 1), 0.20).with_name("tax_rate")); // B2
    sheet.set_cell(Cell::formula(Coord::new(2, 25), "revenue * tax_rate").with_name("tax_amount")); // C26

    // Far-out vector asset stored in row 10,000 (col 0, row 10,000)
    let mut vec_buf = VectorBuffer::new();
    vec_buf.set_color(ColorRgba::rgb(0, 255, 128));
    vec_buf.rect(0.0, 0.0, 50.0, 50.0);
    let blob_bytes = vec_buf.to_bytes();

    let asset_cell = Cell::blob(Coord::new(0, 10_000), "hud_logo", blob_bytes.clone());
    assert!(asset_cell.is_asset(), "Cell at row 10,000 should automatically be flagged as an asset");
    sheet.set_cell(asset_cell);

    wb.sheets.push(sheet);

    // Compute Dynamic TOC
    let toc = wb.compute_toc();

    // Verify TOC position is at D27 (col 3, row 26), NOT row 10,001!
    assert_eq!(
        toc.extent_col, 2,
        "Active grid extent col should be 2 (col C)"
    );
    assert_eq!(
        toc.extent_row, 25,
        "Active grid extent row should be 25 (row 26)"
    );
    assert_eq!(
        toc.pos,
        Coord::new(3, 26),
        "TOC should be dynamically anchored at D27 (3, 26)"
    );

    // Verify all named items including the far-out asset appear in TOC entries
    let entry_names: Vec<&str> = toc.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(entry_names.contains(&"revenue"));
    assert!(entry_names.contains(&"tax_rate"));
    assert!(entry_names.contains(&"tax_amount"));
    assert!(
        entry_names.contains(&"hud_logo"),
        "TOC should catalog the asset blob"
    );

    let hud_entry = toc.entries.iter().find(|e| e.name == "hud_logo").unwrap();
    assert_eq!(hud_entry.type_name, "blob");
    assert_eq!(hud_entry.byte_size, blob_bytes.len());
    assert_eq!(hud_entry.coord, Coord::new(0, 10_000));

    // Full roundtrip serialization check
    let serialized = serialize_workbook(&wb);
    let deserialized = deserialize_workbook(&serialized).expect("Workbook should roundtrip cleanly");
    let des_sheet = &deserialized.sheets[0];
    let loaded_asset = des_sheet.get_cell(Coord::new(0, 10_000)).expect("Asset cell should be preserved");
    assert_eq!(loaded_asset.name.as_deref(), Some("hud_logo"));
    assert_eq!(loaded_asset.value, CellValue::Blob { bytes: blob_bytes });
}

#[test]
fn test_rasterizer_zoom_and_pan_transform() {
    let mut vec_buf = VectorBuffer::new();
    vec_buf.set_color(ColorRgba::rgb(255, 0, 0));
    vec_buf.set_width(1.0);
    vec_buf.move_to(10.0, 10.0);
    vec_buf.line_to(20.0, 10.0);

    let mut bmp_1x = Bitmap::new(100, 100);
    SubpixelRasterizer::render(&mut bmp_1x, &vec_buf, &Transform2D::identity());
    let lit_1x = bmp_1x.pixels.iter().filter(|&&p| p != 0).count();

    let mut bmp_2x = Bitmap::new(100, 100);
    SubpixelRasterizer::render(&mut bmp_2x, &vec_buf, &Transform2D::new(0.0, 0.0, 2.0));
    let lit_2x = bmp_2x.pixels.iter().filter(|&&p| p != 0).count();

    // At 2x zoom, the stroke length doubles on screen so more pixels should be illuminated
    assert!(
        lit_2x > lit_1x,
        "2x zoom should illuminate more pixels than 1x: {} vs {}",
        lit_2x,
        lit_1x
    );
}

#[test]
fn test_end_to_end_basic_turtle_to_bitmap_raster() {
    let mut sheet = Sheet::new("main", "Main Sheet");
    let owner = Coord::new(0, 0);
    let asset_coord = Coord::parse_a1("A10000").unwrap();

    sheet.set_cell(Cell::blob(asset_coord, "chart_vec", vec![]));

    // BASIC script that uses Logo turtle to generate a data chart/shape and label it
    let script = r#"
        COLOR "green"
        WIDTH 2
        ' Draw a triangular marker
        FOR I = 1 TO 3
            FD 40
            RT 120
        NEXT I
        PU
        FD 20
        PD
        LABEL "123"
        COMMIT_TURTLE("A10000")
    "#;

    let writes = lotus_core::BasicRunner::execute(&mut sheet, owner, script, &[], &[asset_coord], None)
        .expect("BASIC script should execute");
    assert_eq!(writes, 1);

    // Retrieve committed blob
    let cell = sheet.get_cell(asset_coord).unwrap();
    let blob_bytes = match &cell.value {
        CellValue::Blob { bytes } => bytes,
        _ => panic!("Expected blob"),
    };

    let vector_buf = VectorBuffer::from_bytes(blob_bytes).expect("Should decode vector buffer");
    assert!(vector_buf.len() > 5);

    // Rasterize with SubpixelRasterizer into a bitmap buffer
    let mut bmp = Bitmap::new(160, 160);
    bmp.clear(0x00000000);
    // Pan to center the turtle drawing which starts around (0,0) and moves north
    SubpixelRasterizer::render(&mut bmp, &vector_buf, &Transform2D::new(-50.0, -80.0, 1.0));

    let illuminated = bmp.pixels.iter().filter(|&&p| p != 0).count();
    assert!(
        illuminated > 50,
        "Rendered turtle graphic should illuminate pixels, got {}",
        illuminated
    );

    // Verify BMP file export
    let bmp_bytes = bmp.to_bmp_bytes();
    assert_eq!(&bmp_bytes[0..2], b"BM");
}

#[test]
fn test_gui_scene_projection() {
    let font = HersheyFont::load_simplex();
    let mut scene = VectorBuffer::new();
    
    // Create a window
    let win = lotus_core::view::gui::GeoWorks::window(10.0, 10.0, 100.0, 50.0, "TEST", &font);
    scene.ops.extend(win.ops);

    let mut bitmap = Bitmap::new(200, 200);
    bitmap.clear(0);
    
    SubpixelRasterizer::render(&mut bitmap, &scene, &Transform2D::identity());
    
    let lit_pixels = bitmap.pixels.iter().filter(|&&p| p != 0).count();
    assert!(lit_pixels > 100, "GUI window should produce visible pixels");
}
