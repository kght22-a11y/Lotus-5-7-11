use super::*;

fn c(col: u32, row: u32) -> Coord {
    Coord::new(col, row)
}

fn sheet_with(cells: &[(Coord, CellValue)]) -> Sheet {
    let mut sheet = Sheet::new("s", "S");
    for (coord, value) in cells {
        sheet.set_cell(Cell::new(*coord, value.clone()));
    }
    recalc(&mut sheet);
    sheet
}

fn num(n: f64) -> CellValue {
    CellValue::Number(n)
}

fn formula(expr: &str) -> CellValue {
    CellValue::parse_input(expr)
}

fn expr_at(sheet: &Sheet, at: Coord) -> String {
    match &sheet.get_cell(at).expect("cell present").value {
        CellValue::Formula { expr, .. } => expr.clone(),
        other => panic!("not a formula: {other:?}"),
    }
}

fn result_at(sheet: &Sheet, at: Coord) -> Option<f64> {
    match &sheet.get_cell(at)?.value {
        CellValue::Formula { cached_num, .. } => *cached_num,
        CellValue::Number(n) => Some(*n),
        _ => None,
    }
}

// ---- ranges ---------------------------------------------------------------

#[test]
fn range_normalizes_corners_and_describes_itself() {
    let r = Range::new(c(3, 5), c(1, 2));
    assert_eq!((r.tl, r.br), (c(1, 2), c(3, 5)));
    assert_eq!((r.cols(), r.rows()), (3, 4));
    assert!(r.contains(c(2, 3)) && !r.contains(c(0, 3)) && !r.contains(c(2, 6)));
    assert_eq!(r.to_a1(), "B3:D6");
    assert_eq!(Range::cell(c(0, 0)).to_a1(), "A1");
    assert!(Range::cell(c(4, 4)).is_single());
}

#[test]
fn used_range_ignores_the_asset_zone() {
    let mut sheet = sheet_with(&[(c(2, 4), num(1.0))]);
    sheet.set_cell(Cell::new(c(0, 20_000), CellValue::Blob { bytes: vec![1, 2, 3] }));
    assert_eq!(used_range(&sheet), Some(Range::new(c(0, 0), c(2, 4))));
    assert_eq!(used_range(&Sheet::new("e", "E")), None);
}

#[test]
fn clear_range_removes_only_cells_inside() {
    let mut sheet = sheet_with(&[(c(0, 0), num(1.0)), (c(1, 1), num(2.0)), (c(3, 3), num(3.0))]);
    clear_range(&mut sheet, Range::new(c(0, 0), c(1, 1)));
    assert_eq!(sheet.cells.len(), 1);
    assert!(sheet.get_cell(c(3, 3)).is_some());
}

// ---- insert / delete --------------------------------------------------------

#[test]
fn inserting_a_row_shifts_cells_and_repairs_formulas() {
    let mut sheet = sheet_with(&[
        (c(0, 0), num(1.0)),
        (c(0, 1), num(2.0)),
        (c(0, 2), formula("=A1+A2")),
    ]);
    insert_lines(&mut sheet, Axis::Row, 1, 1).unwrap();

    assert!(result_at(&sheet, c(0, 0)) == Some(1.0));
    assert!(sheet.get_cell(c(0, 1)).is_none(), "the new row is empty");
    assert_eq!(result_at(&sheet, c(0, 2)), Some(2.0));
    assert_eq!(expr_at(&sheet, c(0, 3)), "=A1+A3");
    assert_eq!(result_at(&sheet, c(0, 3)), Some(3.0), "recalculated after the move");
    assert_eq!(sheet.get_cell(c(0, 3)).unwrap().coord, c(0, 3), "cell knows its new place");
}

#[test]
fn inserting_inside_a_range_grows_it() {
    let mut sheet = sheet_with(&[
        (c(0, 0), num(1.0)),
        (c(0, 1), num(2.0)),
        (c(0, 2), num(3.0)),
        (c(0, 3), formula("=SUM(A1:A3)")),
    ]);
    insert_lines(&mut sheet, Axis::Row, 1, 1).unwrap();
    assert_eq!(expr_at(&sheet, c(0, 4)), "=SUM(A1:A4)");
    assert_eq!(result_at(&sheet, c(0, 4)), Some(6.0));
}

#[test]
fn deleting_a_row_shrinks_ranges_and_breaks_lone_references() {
    let mut sheet = sheet_with(&[
        (c(0, 0), num(1.0)),
        (c(0, 1), num(2.0)),
        (c(0, 2), num(3.0)),
        (c(0, 3), formula("=SUM(A1:A3)")),
        (c(2, 0), formula("=A2")),
    ]);
    delete_lines(&mut sheet, Axis::Row, 1, 1).unwrap();

    assert_eq!(result_at(&sheet, c(0, 1)), Some(3.0), "A3 moved up to A2");
    assert_eq!(expr_at(&sheet, c(0, 2)), "=SUM(A1:A2)");
    assert_eq!(result_at(&sheet, c(0, 2)), Some(4.0));
    assert_eq!(expr_at(&sheet, c(2, 0)), "=#REF!", "the cell it pointed at is gone");
}

#[test]
fn deleting_the_end_of_a_range_pulls_it_back() {
    let mut sheet = sheet_with(&[
        (c(0, 0), num(1.0)),
        (c(0, 1), num(2.0)),
        (c(0, 2), num(3.0)),
        (c(2, 0), formula("=SUM(A1:A3)")),
    ]);
    delete_lines(&mut sheet, Axis::Row, 2, 1).unwrap();
    assert_eq!(expr_at(&sheet, c(2, 0)), "=SUM(A1:A2)");
}

#[test]
fn deleting_everything_a_range_covered_gives_ref_error() {
    let mut sheet = sheet_with(&[
        (c(0, 1), num(1.0)),
        (c(0, 2), num(2.0)),
        (c(2, 0), formula("=SUM(A2:A3)")),
    ]);
    delete_lines(&mut sheet, Axis::Row, 1, 2).unwrap();
    assert_eq!(expr_at(&sheet, c(2, 0)), "=SUM(#REF!)");
}

#[test]
fn names_travel_with_their_cells() {
    let mut sheet = Sheet::new("s", "S");
    sheet.set_cell(Cell::number(c(0, 3), 7.0).with_name("rate"));
    sheet.set_cell(Cell::formula(c(1, 0), "=rate*2"));
    recalc(&mut sheet);
    insert_lines(&mut sheet, Axis::Row, 0, 2).unwrap();
    let moved = sheet.get_cell(c(0, 5)).unwrap();
    assert_eq!(moved.name.as_deref(), Some("rate"));
    assert_eq!(expr_at(&sheet, c(1, 2)), "=rate*2");
    assert_eq!(result_at(&sheet, c(1, 2)), Some(14.0));
}

#[test]
fn row_heights_follow_inserts_and_deletes() {
    let mut sheet = sheet_with(&[(c(0, 5), num(1.0))]);
    sheet.set_row_height(5, 40.0);
    insert_lines(&mut sheet, Axis::Row, 2, 3).unwrap();
    assert_eq!(sheet.row_height(8), 40.0);
    assert_eq!(sheet.row_height(5), sheet.default_row_height);
    delete_lines(&mut sheet, Axis::Row, 8, 1).unwrap();
    assert_eq!(sheet.row_heights.len(), 0, "the deleted row's height is gone");
}

#[test]
fn columns_work_the_same_way() {
    let mut sheet = sheet_with(&[
        (c(0, 0), num(1.0)),
        (c(1, 0), num(2.0)),
        (c(2, 0), formula("=A1+B1")),
    ]);
    sheet.set_col_width(1, 150.0);
    insert_lines(&mut sheet, Axis::Col, 1, 1).unwrap();
    assert_eq!(expr_at(&sheet, c(3, 0)), "=A1+C1");
    assert_eq!(result_at(&sheet, c(3, 0)), Some(3.0));
    assert_eq!(sheet.col_width(2), 150.0);

    delete_lines(&mut sheet, Axis::Col, 1, 1).unwrap();
    assert_eq!(expr_at(&sheet, c(2, 0)), "=A1+B1");
    assert_eq!(sheet.col_width(1), 150.0);
}

#[test]
fn the_asset_zone_never_shifts() {
    let mut sheet = sheet_with(&[(c(0, 0), num(1.0))]);
    sheet.set_cell(Cell::new(c(0, 10_000), CellValue::Blob { bytes: vec![9] }));
    insert_lines(&mut sheet, Axis::Row, 0, 5).unwrap();
    assert!(sheet.get_cell(c(0, 10_000)).is_some());
    assert!(sheet.get_cell(c(0, 5)).is_some());
    delete_lines(&mut sheet, Axis::Row, 0, 5).unwrap();
    assert!(sheet.get_cell(c(0, 10_000)).is_some());
}

#[test]
fn an_insert_that_would_push_cells_into_the_asset_zone_is_refused_untouched() {
    let mut sheet = sheet_with(&[(c(0, 0), num(1.0)), (c(0, 9_999), num(2.0))]);
    let before = sheet.clone();
    let err = insert_lines(&mut sheet, Axis::Row, 0, 1).unwrap_err();
    assert!(err.contains("pushed past"), "{err}");
    assert_eq!(sheet, before);
}

// ---- fill -------------------------------------------------------------------

#[test]
fn fill_down_copies_the_top_row_with_relative_formulas() {
    let mut sheet = sheet_with(&[(c(0, 0), num(5.0)), (c(1, 0), formula("=A1*2"))]);
    fill_down(&mut sheet, Range::new(c(0, 0), c(1, 3))).unwrap();
    for row in 1..=3 {
        assert_eq!(result_at(&sheet, c(0, row)), Some(5.0));
        assert_eq!(expr_at(&sheet, c(1, row)), format!("=A{}*2", row + 1));
        assert_eq!(result_at(&sheet, c(1, row)), Some(10.0));
    }
}

#[test]
fn fill_down_on_one_row_copies_the_row_above() {
    let mut sheet = sheet_with(&[(c(1, 0), num(3.0)), (c(1, 1), num(99.0))]);
    fill_down(&mut sheet, Range::cell(c(1, 1))).unwrap();
    assert_eq!(result_at(&sheet, c(1, 1)), Some(3.0));
    assert!(fill_down(&mut sheet, Range::cell(c(1, 0))).is_err(), "nothing above row 1");
}

#[test]
fn fill_right_copies_the_left_column() {
    let mut sheet = sheet_with(&[(c(0, 0), num(2.0)), (c(0, 1), formula("=A1+1"))]);
    fill_right(&mut sheet, Range::new(c(0, 0), c(3, 1))).unwrap();
    assert_eq!(result_at(&sheet, c(3, 0)), Some(2.0));
    assert_eq!(expr_at(&sheet, c(2, 1)), "=C1+1");
    assert!(fill_right(&mut sheet, Range::cell(c(0, 0))).is_err());
}

#[test]
fn filling_from_an_empty_cell_clears_the_destination() {
    let mut sheet = sheet_with(&[(c(0, 1), num(7.0))]);
    fill_down(&mut sheet, Range::new(c(0, 0), c(0, 1))).unwrap();
    assert!(sheet.get_cell(c(0, 1)).is_none());
}

// ---- sort -------------------------------------------------------------------

#[test]
fn sorting_moves_rows_and_keeps_same_row_formulas_pointing_home() {
    let mut sheet = sheet_with(&[
        (c(0, 0), num(3.0)),
        (c(0, 1), num(1.0)),
        (c(0, 2), num(2.0)),
        (c(1, 0), formula("=A1*10")),
        (c(1, 1), formula("=A2*10")),
        (c(1, 2), formula("=A3*10")),
    ]);
    sort_rows(&mut sheet, Range::new(c(0, 0), c(1, 2)), 0, true).unwrap();
    let a: Vec<_> = (0..3).map(|r| result_at(&sheet, c(0, r)).unwrap()).collect();
    let b: Vec<_> = (0..3).map(|r| result_at(&sheet, c(1, r)).unwrap()).collect();
    assert_eq!(a, vec![1.0, 2.0, 3.0]);
    assert_eq!(b, vec![10.0, 20.0, 30.0]);
    assert_eq!(expr_at(&sheet, c(1, 0)), "=A1*10");

    sort_rows(&mut sheet, Range::new(c(0, 0), c(1, 2)), 0, false).unwrap();
    assert_eq!(result_at(&sheet, c(0, 0)), Some(3.0));
    assert_eq!(result_at(&sheet, c(1, 0)), Some(30.0));
}

#[test]
fn sort_puts_numbers_before_text_and_empties_last_either_way() {
    let build = || {
        sheet_with(&[
            (c(0, 0), CellValue::Text("pear".into())),
            (c(0, 2), num(2.0)),
            (c(0, 3), CellValue::Text("Apple".into())),
            (c(0, 4), num(1.0)),
        ])
    };
    let col = |s: &Sheet| -> Vec<String> {
        (0..5).map(|r| s.get_cell(c(0, r)).map(|x| x.value.display_text()).unwrap_or_default()).collect()
    };
    let mut asc = build();
    sort_rows(&mut asc, Range::new(c(0, 0), c(0, 4)), 0, true).unwrap();
    assert_eq!(col(&asc), vec!["1.00", "2.00", "Apple", "pear", ""]);

    let mut desc = build();
    sort_rows(&mut desc, Range::new(c(0, 0), c(0, 4)), 0, false).unwrap();
    assert_eq!(col(&desc), vec!["pear", "Apple", "2.00", "1.00", ""]);
}

#[test]
fn sort_carries_names_with_their_rows_and_checks_its_input() {
    let mut sheet = Sheet::new("s", "S");
    sheet.set_cell(Cell::number(c(0, 0), 9.0).with_name("big"));
    sheet.set_cell(Cell::number(c(0, 1), 1.0).with_name("small"));
    sort_rows(&mut sheet, Range::new(c(0, 0), c(0, 1)), 0, true).unwrap();
    assert_eq!(sheet.get_cell(c(0, 0)).unwrap().name.as_deref(), Some("small"));

    assert!(sort_rows(&mut sheet, Range::cell(c(0, 0)), 0, true).is_err(), "one row");
    assert!(sort_rows(&mut sheet, Range::new(c(0, 0), c(0, 1)), 5, true).is_err(), "key outside");
}

// ---- clipboard text ---------------------------------------------------------

#[test]
fn range_text_is_tab_separated_rows() {
    let sheet = sheet_with(&[
        (c(0, 0), num(1.0)),
        (c(1, 0), CellValue::Text("a\tb".into())),
        (c(1, 1), formula("=A1+1")),
    ]);
    assert_eq!(range_to_text(&sheet, Range::new(c(0, 0), c(1, 1))), "1\ta b\n\t=A1+1");
}

#[test]
fn pasting_external_text_fills_a_block() {
    let mut sheet = Sheet::new("s", "S");
    let r = paste_text(&mut sheet, c(1, 1), "1\t2\r\n3\ttext\r\n", None).unwrap();
    assert_eq!(r, Range::new(c(1, 1), c(2, 2)));
    assert_eq!(result_at(&sheet, c(2, 1)), Some(2.0));
    assert_eq!(result_at(&sheet, c(1, 2)), Some(3.0));
    assert_eq!(sheet.get_cell(c(2, 2)).unwrap().value, CellValue::Text("text".into()));
    assert_eq!(sheet.cells.len(), 4, "the trailing newline adds no row");
}

#[test]
fn pasting_a_single_value_and_a_blank_overwrites() {
    let mut sheet = sheet_with(&[(c(0, 0), num(1.0)), (c(1, 0), num(2.0))]);
    paste_text(&mut sheet, c(0, 0), "42", None).unwrap();
    assert_eq!(result_at(&sheet, c(0, 0)), Some(42.0));
    paste_text(&mut sheet, c(0, 0), "\t", None).unwrap();
    assert!(sheet.get_cell(c(0, 0)).is_none() && sheet.get_cell(c(1, 0)).is_none());
}

#[test]
fn pasting_what_was_copied_offsets_relative_references() {
    let mut sheet = sheet_with(&[(c(0, 0), num(4.0)), (c(1, 0), formula("=A1*3"))]);
    let text = range_to_text(&sheet, Range::new(c(0, 0), c(1, 0)));
    // paste three rows down and one column right: shift is (+1, +3)
    paste_text(&mut sheet, c(1, 3), &text, Some((1, 3))).unwrap();
    assert_eq!(result_at(&sheet, c(1, 3)), Some(4.0));
    assert_eq!(expr_at(&sheet, c(2, 3)), "=B4*3");
    assert_eq!(result_at(&sheet, c(2, 3)), Some(12.0));
}

#[test]
fn pasting_keeps_the_name_of_the_cell_it_lands_on() {
    let mut sheet = Sheet::new("s", "S");
    sheet.set_cell(Cell::number(c(0, 0), 1.0).with_name("rate"));
    paste_text(&mut sheet, c(0, 0), "5", None).unwrap();
    assert_eq!(sheet.get_cell(c(0, 0)).unwrap().name.as_deref(), Some("rate"));
}

#[test]
fn a_paste_running_off_the_sheet_is_clipped() {
    let mut sheet = Sheet::new("s", "S");
    let r = paste_text(&mut sheet, c(16_383, 0), "1\t2", None).unwrap();
    assert_eq!(sheet.cells.len(), 1);
    assert_eq!(r.br, c(16_383, 0));
}

// ---- search and movement ----------------------------------------------------

#[test]
fn find_next_reads_across_rows_ignores_case_and_wraps() {
    let mut sheet = sheet_with(&[
        (c(0, 0), CellValue::Text("Total".into())),
        (c(2, 0), CellValue::Text("subtotal".into())),
        (c(1, 2), CellValue::Text("TOTALS".into())),
        (c(0, 1), formula("=SUM(B1:B2)")),
    ]);
    sheet.set_cell(Cell::number(c(3, 3), 1.0).with_name("grand_total"));

    assert_eq!(find_next(&sheet, c(0, 0), "total"), Some(c(2, 0)));
    assert_eq!(find_next(&sheet, c(2, 0), "total"), Some(c(1, 2)));
    assert_eq!(find_next(&sheet, c(1, 2), "total"), Some(c(3, 3)), "matches names too");
    assert_eq!(find_next(&sheet, c(3, 3), "total"), Some(c(0, 0)), "wraps around");
    assert_eq!(find_next(&sheet, c(0, 0), "sum("), Some(c(0, 1)), "matches formula text");
    assert_eq!(find_next(&sheet, c(0, 0), "zzz"), None);
    assert_eq!(find_next(&sheet, c(0, 0), ""), None);
}

#[test]
fn jump_edge_follows_blocks_like_a_spreadsheet() {
    let sheet = sheet_with(&[
        (c(0, 0), num(1.0)),
        (c(0, 1), num(1.0)),
        (c(0, 2), num(1.0)),
        (c(0, 5), num(1.0)),
    ]);
    assert_eq!(jump_edge(&sheet, c(0, 0), 0, 1), c(0, 2), "end of the block");
    assert_eq!(jump_edge(&sheet, c(0, 2), 0, 1), c(0, 5), "next filled cell");
    assert_eq!(jump_edge(&sheet, c(0, 5), 0, 1), c(0, 9_999), "sheet edge, never the asset zone");
    assert_eq!(jump_edge(&sheet, c(0, 5), 0, -1), c(0, 2));
    assert_eq!(jump_edge(&sheet, c(0, 0), -1, 0), c(0, 0), "already at the left edge");
    assert_eq!(jump_edge(&sheet, c(0, 0), 1, 0), c(16_383, 0), "empty row runs to the edge");
}

// ---- sizes ------------------------------------------------------------------

#[test]
fn fit_width_tracks_the_widest_value() {
    let sheet = sheet_with(&[
        (c(0, 0), CellValue::Text("hi".into())),
        (c(0, 1), CellValue::Text("a much longer label".into())),
        (c(1, 0), num(1.0)),
    ]);
    let measure = |s: &str| s.len() as f32 * 6.0;
    assert_eq!(fit_col_width(&sheet, 0, &measure), 19.0 * 6.0 + 14.0);
    assert_eq!(fit_col_width(&sheet, 5, &measure), sheet.default_col_width, "empty column");
    assert_eq!(fit_col_width(&sheet, 1, &|_| 4.0), 40.0, "never below the minimum");
    assert_eq!(fit_col_width(&sheet, 0, &|_| 5_000.0), 600.0, "never above the maximum");
}

#[test]
fn collapsed_means_the_smallest_size() {
    let mut sheet = Sheet::new("s", "S");
    assert!(!is_col_collapsed(&sheet, 0) && !is_row_collapsed(&sheet, 0));
    sheet.set_col_width(0, 1.0);
    sheet.set_row_height(0, 1.0);
    assert!(is_col_collapsed(&sheet, 0) && is_row_collapsed(&sheet, 0));
    assert_eq!(sheet.col_width(0), COLLAPSED_COL_WIDTH);
    assert_eq!(sheet.row_height(0), COLLAPSED_ROW_HEIGHT);
}
