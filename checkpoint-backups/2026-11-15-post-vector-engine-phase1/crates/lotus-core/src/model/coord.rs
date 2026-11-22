//! Spreadsheet grid coordinate system (0-indexed internally, 1-indexed A1 externally).

#[cfg(not(feature = "std"))]
use alloc::string::String;
use core::fmt;

/// Spatial coordinate on a worksheet grid.
/// `col` is 0-indexed (0 = A, 1 = B, 26 = AA, etc.).
/// `row` is 0-indexed (0 = 1, 26 = 27, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Coord {
    pub col: u32,
    pub row: u32,
}

impl Coord {
    pub const fn new(col: u32, row: u32) -> Self {
        Self { col, row }
    }

    /// Parse an A1-notation reference (e.g. "A1", "D27", "AA100") into a `Coord`.
    pub fn parse_a1(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let mut col: u32 = 0;
        let mut row_start = 0;
        let mut has_letters = false;

        for (i, b) in s.bytes().enumerate() {
            if b.is_ascii_alphabetic() {
                has_letters = true;
                let val = (b.to_ascii_uppercase() - b'A' + 1) as u32;
                col = col.checked_mul(26)?.checked_add(val)?;
            } else if b.is_ascii_digit() {
                row_start = i;
                break;
            } else {
                return None;
            }
        }

        if !has_letters || row_start == 0 {
            return None;
        }

        let row_str = &s[row_start..];
        let row_1based: u32 = row_str.parse().ok()?;
        if row_1based == 0 {
            return None;
        }

        Some(Self {
            col: col - 1,
            row: row_1based - 1,
        })
    }

    /// Format this coordinate into standard A1-notation (e.g. `Coord { col: 3, row: 26 }` -> `"D27"`).
    pub fn to_a1(&self) -> String {
        let mut col_str = String::new();
        let mut c = self.col + 1;
        while c > 0 {
            let rem = ((c - 1) % 26) as u8;
            col_str.insert(0, (b'A' + rem) as char);
            c = (c - 1) / 26;
        }

        #[cfg(feature = "std")]
        return format!("{}{}", col_str, self.row + 1);

        #[cfg(not(feature = "std"))]
        {
            use alloc::format;
            format!("{}{}", col_str, self.row + 1)
        }
    }
}

impl fmt::Display for Coord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_a1())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coord_a1_roundtrip() {
        let cases = [
            ("A1", Coord::new(0, 0)),
            ("B2", Coord::new(1, 1)),
            ("Z1", Coord::new(25, 0)),
            ("AA1", Coord::new(26, 0)),
            ("AB10", Coord::new(27, 9)),
            ("D27", Coord::new(3, 26)),
            ("ZZ999", Coord::new(701, 998)),
        ];

        for (a1, coord) in cases {
            assert_eq!(Coord::parse_a1(a1), Some(coord), "Failed parsing {}", a1);
            assert_eq!(coord.to_a1(), a1, "Failed formatting {:?}", coord);
        }
    }
}
