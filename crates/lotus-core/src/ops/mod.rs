//! Spreadsheet operations on one sheet: ranges, insert/delete, fill, sort,
//! clipboard text, search and column fitting.
//!
//! Everything here is a pure function over [`Sheet`]: no window, no keyboard,
//! no OS. The GUI, the terminal view and a bare-metal shell all call the same
//! code, so an operation behaves identically wherever it is triggered.
//!
//! Operations that change cells finish by recalculating the sheet, so a
//! caller cannot forget to.

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, format, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use core::cmp::Ordering;

use crate::{
    calc::{
        offset_refs,
        refs::{MAX_COL, MAX_ROW},
        rewrite_refs, CalcEngine, RefKind,
    },
    model::{Cell, CellValue, Coord, Sheet},
};

/// Rows from here down hold binary/vector assets and never shift
/// (this is the same rule `Cell::is_asset` applies).
pub const ASSET_ROW_START: u32 = 10_000;
/// A collapsed column or row is the smallest size the sheet allows.
pub const COLLAPSED_COL_WIDTH: f32 = 10.0;
pub const COLLAPSED_ROW_HEIGHT: f32 = 8.0;
/// Pasting more cells than this at once is refused.
const MAX_PASTE_CELLS: usize = 1_000_000;

// ---- ranges -------------------------------------------------------------------

/// An inclusive rectangle of cells, always stored top-left to bottom-right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub tl: Coord,
    pub br: Coord,
}

impl Range {
    /// The rectangle spanned by any two opposite corners.
    pub fn new(a: Coord, b: Coord) -> Self {
        Self {
            tl: Coord::new(a.col.min(b.col), a.row.min(b.row)),
            br: Coord::new(a.col.max(b.col), a.row.max(b.row)),
        }
    }

    pub fn cell(c: Coord) -> Self {
        Self { tl: c, br: c }
    }

    pub fn contains(&self, c: Coord) -> bool {
        c.col >= self.tl.col && c.col <= self.br.col && c.row >= self.tl.row && c.row <= self.br.row
    }

    pub fn cols(&self) -> u32 {
        self.br.col - self.tl.col + 1
    }

    pub fn rows(&self) -> u32 {
        self.br.row - self.tl.row + 1
    }

    pub fn is_single(&self) -> bool {
        self.tl == self.br
    }

    /// `A1:C5`, or just `A1` for one cell.
    pub fn to_a1(&self) -> String {
        if self.is_single() {
            self.tl.to_a1()
        } else {
            format!("{}:{}", self.tl.to_a1(), self.br.to_a1())
        }
    }
}

/// Everything in the sheet's active grid, from A1 to the furthest used cell.
pub fn used_range(sheet: &Sheet) -> Option<Range> {
    let (col, row) = sheet.active_grid_bounds()?;
    Some(Range::new(Coord::new(0, 0), Coord::new(col, row)))
}

// ---- setting values -----------------------------------------------------------

/// Put `value` at `dest`, keeping the cell's name and links if it already
/// exists (as typing does). `Empty` removes the cell.
fn set_value(sheet: &mut Sheet, dest: Coord, value: CellValue) {
    if value.is_empty() {
        sheet.cells.remove(&dest);
    } else if let Some(cell) = sheet.cells.get_mut(&dest) {
        cell.value = value;
    } else {
        sheet.set_cell(Cell::new(dest, value));
    }
}

/// A copy of `value` as if moved by (`d_col`, `d_row`): formulas have their
/// references offset and their cached results dropped.
fn shifted_value(value: &CellValue, d_col: i64, d_row: i64) -> CellValue {
    match value {
        CellValue::Formula { expr, .. } => CellValue::Formula {
            expr: offset_refs(expr, d_col, d_row),
            cached_num: None,
            cached_text: None,
            cached_bool: None,
        },
        other => other.clone(),
    }
}

fn recalc(sheet: &mut Sheet) {
    let _ = CalcEngine::recalculate_sheet(sheet, None);
}

/// Remove every cell in `range`.
pub fn clear_range(sheet: &mut Sheet, range: Range) {
    let doomed: Vec<Coord> = sheet.cells.keys().copied().filter(|c| range.contains(*c)).collect();
    for coord in doomed {
        sheet.cells.remove(&coord);
    }
    recalc(sheet);
}

// ---- insert / delete rows and columns ----------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Row,
    Col,
}

impl Axis {
    fn get(self, c: Coord) -> u32 {
        match self {
            Axis::Row => c.row,
            Axis::Col => c.col,
        }
    }

    fn with(self, c: Coord, v: u32) -> Coord {
        match self {
            Axis::Row => Coord::new(c.col, v),
            Axis::Col => Coord::new(v, c.row),
        }
    }

    /// First index that does not shift: the asset zone for rows, the sheet edge for columns.
    fn limit(self) -> u32 {
        match self {
            Axis::Row => ASSET_ROW_START,
            Axis::Col => MAX_COL + 1,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Axis::Row => "row",
            Axis::Col => "column",
        }
    }
}

/// Where index `v` ends up when `count` lines are inserted at `at`.
fn map_insert(v: u32, at: u32, count: u32, limit: u32) -> Option<u32> {
    if v < at || v >= limit {
        Some(v)
    } else {
        let nv = v + count;
        if nv >= limit { None } else { Some(nv) }
    }
}

/// Where index `v` ends up when `count` lines are deleted at `at`. A deleted
/// line is `None` for a lone cell; for a range, its start slides to the first
/// surviving line and its end back to the last one, so the range shrinks.
fn map_delete(v: u32, at: u32, count: u32, limit: u32, kind: RefKind) -> Option<u32> {
    if v < at || v >= limit {
        Some(v)
    } else if v >= at + count {
        Some(v - count)
    } else {
        match kind {
            RefKind::Cell => None,
            RefKind::RangeStart => Some(at),
            RefKind::RangeEnd => at.checked_sub(1),
        }
    }
}

fn shift_lines(sheet: &mut Sheet, axis: Axis, at: u32, count: u32, insert: bool) -> Result<(), String> {
    let limit = axis.limit();
    if count == 0 || at >= limit {
        return Ok(());
    }
    let count = if insert { count } else { count.min(limit - at) };

    if insert {
        let blocked = sheet.cells.keys().any(|c| {
            let v = axis.get(*c);
            v >= at && v < limit && v + count >= limit
        });
        if blocked {
            return Err(format!("Cannot insert: cells would be pushed past the end of the {}s", axis.name()));
        }
    }

    // Move the cells.
    let old = core::mem::take(&mut sheet.cells);
    for (coord, mut cell) in old {
        let v = axis.get(coord);
        let nv = if insert {
            map_insert(v, at, count, limit)
        } else {
            map_delete(v, at, count, limit, RefKind::Cell)
        };
        if let Some(nv) = nv {
            let moved = axis.with(coord, nv);
            cell.coord = moved;
            sheet.cells.insert(moved, cell);
        }
    }

    // Move the per-line sizes.
    let sizes = match axis {
        Axis::Row => &mut sheet.row_heights,
        Axis::Col => &mut sheet.col_widths,
    };
    let old_sizes = core::mem::take(sizes);
    let mut new_sizes: BTreeMap<u32, f32> = BTreeMap::new();
    for (v, size) in old_sizes {
        let nv = if insert {
            map_insert(v, at, count, limit)
        } else {
            map_delete(v, at, count, limit, RefKind::Cell)
        };
        if let Some(nv) = nv {
            new_sizes.insert(nv, size);
        }
    }
    *sizes = new_sizes;

    // Repair formulas.
    for cell in sheet.cells.values_mut() {
        if let CellValue::Formula { expr, cached_num, cached_text, cached_bool } = &mut cell.value {
            *expr = rewrite_refs(expr, &mut |c, kind| {
                let v = axis.get(c);
                let nv = if insert {
                    map_insert(v, at, count, limit)
                } else {
                    map_delete(v, at, count, limit, kind)
                };
                nv.map(|nv| axis.with(c, nv))
            });
            *cached_num = None;
            *cached_text = None;
            *cached_bool = None;
        }
    }

    recalc(sheet);
    Ok(())
}

/// Insert `count` empty rows or columns before index `at`.
pub fn insert_lines(sheet: &mut Sheet, axis: Axis, at: u32, count: u32) -> Result<(), String> {
    shift_lines(sheet, axis, at, count, true)
}

/// Delete `count` rows or columns starting at `at`. Formulas that pointed into
/// the deleted block become `#REF!`; ranges shrink.
pub fn delete_lines(sheet: &mut Sheet, axis: Axis, at: u32, count: u32) -> Result<(), String> {
    shift_lines(sheet, axis, at, count, false)
}

// ---- fill ---------------------------------------------------------------------

fn copy_value(sheet: &mut Sheet, src: Coord, dest: Coord) {
    let d_col = i64::from(dest.col) - i64::from(src.col);
    let d_row = i64::from(dest.row) - i64::from(src.row);
    let value = sheet
        .get_cell(src)
        .map(|c| shifted_value(&c.value, d_col, d_row))
        .unwrap_or(CellValue::Empty);
    set_value(sheet, dest, value);
}

/// Copy the top row of `range` down through it. With a single-row range, copy
/// the row above instead. Formulas keep their relative references.
pub fn fill_down(sheet: &mut Sheet, range: Range) -> Result<(), String> {
    let (src_row, first_dest) = if range.rows() == 1 {
        if range.tl.row == 0 {
            return Err(String::from("Nothing above to fill from"));
        }
        (range.tl.row - 1, range.tl.row)
    } else {
        (range.tl.row, range.tl.row + 1)
    };
    for col in range.tl.col..=range.br.col {
        for row in first_dest..=range.br.row {
            copy_value(sheet, Coord::new(col, src_row), Coord::new(col, row));
        }
    }
    recalc(sheet);
    Ok(())
}

/// Copy the left column of `range` right through it. With a single-column
/// range, copy the column to its left instead.
pub fn fill_right(sheet: &mut Sheet, range: Range) -> Result<(), String> {
    let (src_col, first_dest) = if range.cols() == 1 {
        if range.tl.col == 0 {
            return Err(String::from("Nothing to the left to fill from"));
        }
        (range.tl.col - 1, range.tl.col)
    } else {
        (range.tl.col, range.tl.col + 1)
    };
    for row in range.tl.row..=range.br.row {
        for col in first_dest..=range.br.col {
            copy_value(sheet, Coord::new(src_col, row), Coord::new(col, row));
        }
    }
    recalc(sheet);
    Ok(())
}

// ---- sort ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum SortKey {
    Empty,
    Num(f64),
    Text(String),
}

fn sort_key(cell: Option<&Cell>) -> SortKey {
    match cell.map(|c| &c.value) {
        Some(CellValue::Number(n)) => SortKey::Num(*n),
        Some(CellValue::Boolean(b)) => SortKey::Num(if *b { 1.0 } else { 0.0 }),
        Some(CellValue::Text(t)) => SortKey::Text(t.to_ascii_lowercase()),
        Some(CellValue::Formula { cached_num, cached_bool, cached_text, .. }) => {
            if let Some(n) = cached_num {
                SortKey::Num(*n)
            } else if let Some(b) = cached_bool {
                SortKey::Num(if *b { 1.0 } else { 0.0 })
            } else if let Some(t) = cached_text {
                SortKey::Text(t.to_ascii_lowercase())
            } else {
                SortKey::Empty
            }
        }
        _ => SortKey::Empty,
    }
}

/// Empty cells always sort last; numbers come before text when ascending.
fn compare_keys(a: &SortKey, b: &SortKey, ascending: bool) -> Ordering {
    let order = |o: Ordering| if ascending { o } else { o.reverse() };
    match (a, b) {
        (SortKey::Empty, SortKey::Empty) => Ordering::Equal,
        (SortKey::Empty, _) => Ordering::Greater,
        (_, SortKey::Empty) => Ordering::Less,
        (SortKey::Num(x), SortKey::Num(y)) => order(x.partial_cmp(y).unwrap_or(Ordering::Equal)),
        (SortKey::Text(x), SortKey::Text(y)) => order(x.cmp(y)),
        (SortKey::Num(_), SortKey::Text(_)) => order(Ordering::Less),
        (SortKey::Text(_), SortKey::Num(_)) => order(Ordering::Greater),
    }
}

/// Sort the rows of `range` by the values in column `key_col` (which must be
/// inside the range). Rows move as a unit, with their names and links, and
/// each formula's relative references follow the row.
pub fn sort_rows(sheet: &mut Sheet, range: Range, key_col: u32, ascending: bool) -> Result<(), String> {
    if key_col < range.tl.col || key_col > range.br.col {
        return Err(String::from("The sort column is outside the selection"));
    }
    if range.rows() < 2 {
        return Err(String::from("Select at least two rows to sort"));
    }

    let mut rows: Vec<(u32, SortKey, Vec<Cell>)> = Vec::new();
    for row in range.tl.row..=range.br.row {
        let key = sort_key(sheet.get_cell(Coord::new(key_col, row)));
        let mut cells = Vec::new();
        for col in range.tl.col..=range.br.col {
            if let Some(cell) = sheet.cells.remove(&Coord::new(col, row)) {
                cells.push(cell);
            }
        }
        rows.push((row, key, cells));
    }

    rows.sort_by(|a, b| compare_keys(&a.1, &b.1, ascending));

    for (index, (old_row, _, cells)) in rows.into_iter().enumerate() {
        let new_row = range.tl.row + index as u32;
        let d_row = i64::from(new_row) - i64::from(old_row);
        for mut cell in cells {
            cell.value = shifted_value(&cell.value, 0, d_row);
            cell.coord = Coord::new(cell.coord.col, new_row);
            sheet.cells.insert(cell.coord, cell);
        }
    }

    recalc(sheet);
    Ok(())
}

// ---- clipboard text ---------------------------------------------------------

/// The text a cell puts on the clipboard, or `None` when it has nothing to copy.
pub fn cell_clipboard_text(value: &CellValue) -> Option<String> {
    let text = match value {
        CellValue::Number(n) => format!("{}", n),
        CellValue::Text(t) => t.clone(),
        CellValue::Formula { expr, .. } => expr.clone(),
        CellValue::Boolean(b) => String::from(if *b { "TRUE" } else { "FALSE" }),
        _ => String::new(),
    };
    if text.is_empty() { None } else { Some(text) }
}

/// A range as tab-separated rows, the plain-text form spreadsheets exchange.
pub fn range_to_text(sheet: &Sheet, range: Range) -> String {
    let mut out = String::new();
    for row in range.tl.row..=range.br.row {
        if row > range.tl.row {
            out.push('\n');
        }
        for col in range.tl.col..=range.br.col {
            if col > range.tl.col {
                out.push('\t');
            }
            if let Some(text) = sheet.get_cell(Coord::new(col, row)).and_then(|c| cell_clipboard_text(&c.value)) {
                // A tab or newline inside a cell would break the grid shape.
                out.extend(text.chars().map(|ch| if ch == '\t' || ch == '\n' || ch == '\r' { ' ' } else { ch }));
            }
        }
    }
    out
}

/// Paste tab-separated text with its top-left at `origin`. `shift` is the
/// distance the text was copied across, so pasted formulas keep their relative
/// references; pass `None` for text that came from outside.
pub fn paste_text(sheet: &mut Sheet, origin: Coord, text: &str, shift: Option<(i64, i64)>) -> Result<Range, String> {
    let mut lines: Vec<&str> = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    if lines.len() > 1 && lines.last().map_or(false, |l| l.is_empty()) {
        lines.pop(); // spreadsheets end a copied block with a newline
    }
    let width = lines.iter().map(|l| l.split('\t').count()).max().unwrap_or(1);
    if lines.len().saturating_mul(width) > MAX_PASTE_CELLS {
        return Err(String::from("Too much to paste at once"));
    }

    let mut last = origin;
    for (r, line) in lines.iter().enumerate() {
        for (c, field) in line.split('\t').enumerate() {
            let col = u64::from(origin.col) + c as u64;
            let row = u64::from(origin.row) + r as u64;
            if col > u64::from(MAX_COL) || row > u64::from(MAX_ROW) {
                continue;
            }
            let dest = Coord::new(col as u32, row as u32);
            let mut value = CellValue::parse_input(field);
            if let (CellValue::Formula { .. }, Some((dc, dr))) = (&value, shift) {
                value = shifted_value(&value, dc, dr);
            }
            set_value(sheet, dest, value);
            last = Coord::new(last.col.max(dest.col), last.row.max(dest.row));
        }
    }
    recalc(sheet);
    Ok(Range::new(origin, last))
}

// ---- search and movement ----------------------------------------------------

fn cell_matches(cell: &Cell, needle_lower: &str) -> bool {
    if cell.value.display_text().to_ascii_lowercase().contains(needle_lower) {
        return true;
    }
    if let CellValue::Formula { expr, .. } = &cell.value {
        if expr.to_ascii_lowercase().contains(needle_lower) {
            return true;
        }
    }
    cell.name.as_deref().map_or(false, |n| n.to_ascii_lowercase().contains(needle_lower))
}

/// The next cell, reading across rows left to right, whose shown value,
/// formula or name contains `needle` (ignoring case). Wraps around the sheet.
pub fn find_next(sheet: &Sheet, from: Coord, needle: &str) -> Option<Coord> {
    if needle.is_empty() {
        return None;
    }
    let needle = needle.to_ascii_lowercase();
    let mut hits: Vec<(u32, u32)> = sheet
        .cells
        .values()
        .filter(|c| !c.is_asset() && cell_matches(c, &needle))
        .map(|c| (c.coord.row, c.coord.col))
        .collect();
    hits.sort_unstable();
    let after = hits.iter().find(|h| **h > (from.row, from.col)).or_else(|| hits.first())?;
    Some(Coord::new(after.1, after.0))
}

/// Ctrl+Arrow: from `from`, step by (`d_col`, `d_row`) to the edge of the
/// current block of data, or to the next filled cell, or to the sheet edge.
pub fn jump_edge(sheet: &Sheet, from: Coord, d_col: i32, d_row: i32) -> Coord {
    let filled = |c: Coord| sheet.get_cell(c).map_or(false, |x| !x.value.is_empty());
    let step = |c: Coord| -> Option<Coord> {
        let col = i64::from(c.col) + i64::from(d_col);
        let row = i64::from(c.row) + i64::from(d_row);
        if (0..=i64::from(MAX_COL)).contains(&col) && (0..i64::from(ASSET_ROW_START)).contains(&row) {
            Some(Coord::new(col as u32, row as u32))
        } else {
            None
        }
    };

    let Some(mut cur) = step(from) else { return from };
    if filled(from) && filled(cur) {
        while let Some(next) = step(cur) {
            if filled(next) {
                cur = next;
            } else {
                break;
            }
        }
    } else {
        while !filled(cur) {
            match step(cur) {
                Some(next) => cur = next,
                None => break,
            }
        }
    }
    cur
}

// ---- column and row sizes ---------------------------------------------------

/// The width that fits column `col`'s widest value, given a text-width
/// measure. Empty columns get the sheet's default width.
pub fn fit_col_width(sheet: &Sheet, col: u32, measure: &dyn Fn(&str) -> f32) -> f32 {
    let mut widest = 0.0_f32;
    for cell in sheet.cells.values() {
        if cell.coord.col == col && !cell.is_asset() {
            let w = measure(&cell.value.display_text());
            if w > widest {
                widest = w;
            }
        }
    }
    if widest <= 0.0 {
        sheet.default_col_width
    } else {
        (widest + 14.0).clamp(40.0, 600.0)
    }
}

pub fn is_col_collapsed(sheet: &Sheet, col: u32) -> bool {
    sheet.col_width(col) <= COLLAPSED_COL_WIDTH
}

pub fn is_row_collapsed(sheet: &Sheet, row: u32) -> bool {
    sheet.row_height(row) <= COLLAPSED_ROW_HEIGHT
}

#[cfg(test)]
mod tests;
