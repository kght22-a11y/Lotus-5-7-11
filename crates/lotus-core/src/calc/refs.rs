//! Rewriting cell references inside formula text.
//!
//! Formulas are stored as text (`=A1*B2`), so anything that moves cells
//! (insert or delete rows and columns, fill, paste, sort) has to fix up the
//! references inside them. This works on the text itself and leaves everything
//! else exactly as typed: spacing, string literals, function names and named
//! references are never touched.
//!
//! What counts as a reference follows the lexer: an identifier that parses as
//! A1 (`Coord::parse_a1`) and is not followed by `(`. `A1:B2` is a range.

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::model::Coord;

pub const MAX_COL: u32 = 16_383;
pub const MAX_ROW: u32 = 1_048_575;

/// What a rewritten reference is: a lone cell, or one end of a range.
/// Deletions use this to shrink a range instead of breaking it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    Cell,
    RangeStart,
    RangeEnd,
}

const REF_ERROR: &str = "#REF!";

enum Seg<'a> {
    Lit(&'a str),
    Ref(Coord),
}

fn segments(formula: &str) -> Vec<Seg<'_>> {
    let bytes = formula.as_bytes();
    let mut segs = Vec::new();
    let mut lit_start = 0;
    let mut i = 0;
    let mut in_string = false;

    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        if b.is_ascii_alphabetic() || b == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$') {
                i += 1;
            }
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let is_call = j < bytes.len() && bytes[j] == b'(';
            if !is_call {
                if let Some(coord) = Coord::parse_a1(&formula[start..i]) {
                    segs.push(Seg::Lit(&formula[lit_start..start]));
                    segs.push(Seg::Ref(coord));
                    lit_start = i;
                }
            }
            continue;
        }
        i += 1;
    }
    segs.push(Seg::Lit(&formula[lit_start..]));
    segs
}

/// Rewrite every cell reference in `formula` through `map`.
///
/// `map` returns the new coordinate, or `None` when the referenced cell no
/// longer exists; that reference becomes `#REF!`. A range whose ends no longer
/// line up (everything it covered was deleted) also becomes `#REF!`.
pub fn rewrite_refs(formula: &str, map: &mut dyn FnMut(Coord, RefKind) -> Option<Coord>) -> String {
    let segs = segments(formula);
    let mut out = String::with_capacity(formula.len() + 8);
    let mut k = 0;

    while k < segs.len() {
        match &segs[k] {
            Seg::Lit(text) => {
                out.push_str(text);
                k += 1;
            }
            Seg::Ref(a) => {
                let range_tail = match (segs.get(k + 1), segs.get(k + 2)) {
                    (Some(Seg::Lit(mid)), Some(Seg::Ref(b))) if mid.trim() == ":" => Some((*mid, *b)),
                    _ => None,
                };
                if let Some((mid, b)) = range_tail {
                    let was_ordered = a.col <= b.col && a.row <= b.row;
                    let new_a = map(*a, RefKind::RangeStart);
                    let new_b = map(b, RefKind::RangeEnd);
                    match (new_a, new_b) {
                        (Some(x), Some(y)) if !was_ordered || (x.col <= y.col && x.row <= y.row) => {
                            out.push_str(&x.to_a1());
                            out.push_str(mid);
                            out.push_str(&y.to_a1());
                        }
                        _ => out.push_str(REF_ERROR),
                    }
                    k += 3;
                } else {
                    match map(*a, RefKind::Cell) {
                        Some(c) => out.push_str(&c.to_a1()),
                        None => out.push_str(REF_ERROR),
                    }
                    k += 1;
                }
            }
        }
    }
    out
}

/// Move every reference by a fixed offset, as copy/paste, fill and sort do.
/// A reference pushed off the sheet becomes `#REF!`.
pub fn offset_refs(formula: &str, d_col: i64, d_row: i64) -> String {
    rewrite_refs(formula, &mut |c, _| {
        let col = i64::from(c.col) + d_col;
        let row = i64::from(c.row) + d_row;
        if (0..=i64::from(MAX_COL)).contains(&col) && (0..=i64::from(MAX_ROW)).contains(&row) {
            Some(Coord::new(col as u32, row as u32))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_moves_every_reference_and_keeps_spacing() {
        assert_eq!(offset_refs("=A1 * B2", 1, 1), "=B2 * C3");
        assert_eq!(offset_refs("=A1+A2", 0, 3), "=A4+A5");
        assert_eq!(offset_refs("=B3-A1", -1, -1), "=A2-#REF!");
    }

    #[test]
    fn offset_moves_both_ends_of_a_range() {
        assert_eq!(offset_refs("=SUM(A1:A5)", 2, 0), "=SUM(C1:C5)");
        assert_eq!(offset_refs("=SUM(A1 : B2)", 0, 1), "=SUM(A2 : B3)");
    }

    #[test]
    fn function_names_strings_and_names_are_left_alone() {
        // LOG10 looks like a cell (column LOG, row 10) but is followed by '('.
        assert_eq!(offset_refs("=LOG10(A1)", 0, 1), "=LOG10(A2)");
        assert_eq!(offset_refs("=\"A1\"&B1", 1, 0), "=\"A1\"&C1");
        assert_eq!(offset_refs("=revenue * tax_rate", 5, 5), "=revenue * tax_rate");
        assert_eq!(offset_refs("=IF(TRUE, A1, 0)", 0, 1), "=IF(TRUE, A2, 0)");
    }

    #[test]
    fn a_reference_pushed_off_the_sheet_becomes_ref_error() {
        assert_eq!(offset_refs("=A1", -1, 0), "=#REF!");
        assert_eq!(offset_refs("=A1+B1", -1, 0), "=#REF!+A1");
        assert_eq!(offset_refs("=SUM(A1:B2)", 0, -1), "=SUM(#REF!)");
    }

    #[test]
    fn rewrite_can_shrink_a_range_by_kind() {
        // Pretend row 5 (zero-based 4) was deleted: the end moves up one.
        let out = rewrite_refs("=SUM(A1:A5)", &mut |c, kind| match (c.row, kind) {
            (4, RefKind::RangeEnd) => Some(Coord::new(c.col, 3)),
            _ => Some(c),
        });
        assert_eq!(out, "=SUM(A1:A4)");
    }

    #[test]
    fn an_inverted_range_is_a_ref_error() {
        let out = rewrite_refs("=SUM(A2:A3)", &mut |c, kind| match kind {
            RefKind::RangeStart => Some(Coord::new(c.col, 5)),
            _ => Some(Coord::new(c.col, 1)),
        });
        assert_eq!(out, "=SUM(#REF!)");
    }

    #[test]
    fn lowercase_and_non_ascii_text_survive() {
        assert_eq!(offset_refs("=a1 & \"é\"", 1, 0), "=B1 & \"é\"");
        assert_eq!(offset_refs("", 1, 1), "");
        assert_eq!(offset_refs("=1+2", 1, 1), "=1+2");
    }
}
