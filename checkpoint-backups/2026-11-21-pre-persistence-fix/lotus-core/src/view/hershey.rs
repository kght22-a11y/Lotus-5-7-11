//! Hershey vector stroke font engine.
//!
//! Decodes Dr. A. V. Hershey's 1967 public domain stroke fonts (Simplex Roman)
//! into continuous vector path operations without external dependencies or font libraries.

#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec};

use crate::model::vector::VectorBuffer;

/// Raw embedded Hershey Simplex Roman font definition (JHF format).
pub const HERSHEY_ROMAN_SIMPLEX_JHF: &str = include_str!("rowmans.jhf");

/// A decoded Hershey glyph stroke segment (a continuous pen-down stroke).
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeSegment {
    pub points: Vec<(i8, i8)>,
}

/// A decoded Hershey font glyph with horizontal metrics and stroke segments.
#[derive(Debug, Clone, PartialEq)]
pub struct HersheyGlyph {
    pub glyph_id: u32,
    pub left: i8,
    pub right: i8,
    pub strokes: Vec<StrokeSegment>,
}

impl HersheyGlyph {
    /// Width metric in font units.
    pub fn advance_width(&self) -> f32 {
        (self.right - self.left).max(1) as f32
    }
}

/// A parsed Hershey font set for printable ASCII (32..=126).
#[derive(Debug, Clone)]
pub struct HersheyFont {
    glyphs: [Option<HersheyGlyph>; 128],
}

impl HersheyFont {
    /// Parse and load the built-in Roman Simplex font set.
    pub fn load_simplex() -> Self {
        let mut glyphs: [Option<HersheyGlyph>; 128] = core::array::from_fn(|_| None);
        let mut current_ascii = 32u8; // Starts at Space (' ')

        // JHF lines can wrap long glyphs across lines.
        // A new glyph line begins with 5 chars (digits/spaces) for glyph ID,
        // then 3 chars for pair count, then 1 char for left, 1 for right.
        let mut full_glyph_str = Vec::new();

        for line in HERSHEY_ROMAN_SIMPLEX_JHF.lines() {
            let line = line.trim_end();
            if line.is_empty() {
                continue;
            }

            // Check if this line is a continuation line or a new glyph
            // A new glyph line has at least 10 characters and columns 0..5 are numeric/whitespace ID
            let is_new_glyph = line.len() >= 10
                && line[..5].chars().all(|c| c.is_ascii_digit() || c == ' ')
                && line[5..8].chars().all(|c| c.is_ascii_digit() || c == ' ')
                && line[..5].trim().parse::<u32>().is_ok();

            if is_new_glyph && !full_glyph_str.is_empty() {
                // Parse previous completed glyph
                let combined = full_glyph_str.join("");
                if let Some(glyph) = parse_jhf_glyph(&combined) {
                    if current_ascii < 128 {
                        glyphs[current_ascii as usize] = Some(glyph);
                        current_ascii += 1;
                    }
                }
                full_glyph_str.clear();
            }

            full_glyph_str.push(line);
        }

        // Parse the final glyph
        if !full_glyph_str.is_empty() {
            let combined = full_glyph_str.join("");
            if let Some(glyph) = parse_jhf_glyph(&combined) {
                if current_ascii < 128 {
                    glyphs[current_ascii as usize] = Some(glyph);
                }
            }
        }

        Self { glyphs }
    }

    /// Retrieve the glyph for an ASCII character.
    pub fn get_glyph(&self, ch: char) -> Option<&HersheyGlyph> {
        let byte = ch as usize;
        if byte < 128 {
            self.glyphs[byte].as_ref()
        } else {
            None
        }
    }

    /// Append the strokes of a single character into a vector buffer.
    ///
    /// Returns the horizontal advance width in pixels.
    pub fn draw_char(
        &self,
        ch: char,
        origin_x: f32,
        origin_y: f32,
        scale: f32,
        buf: &mut VectorBuffer,
    ) -> f32 {
        let glyph = match self.get_glyph(ch) {
            Some(g) => g,
            None => {
                // Fallback for missing glyphs: advance equivalent to space
                return 16.0 * scale;
            }
        };

        let left_offset = glyph.left as f32;

        for stroke in &glyph.strokes {
            let mut is_first = true;
            for &(px, py) in &stroke.points {
                // Hershey coordinate origin: X centered at 0, Y goes positive DOWN
                let screen_x = origin_x + (px as f32 - left_offset) * scale;
                let screen_y = origin_y + (py as f32) * scale;

                if is_first {
                    buf.move_to(screen_x, screen_y);
                    is_first = false;
                } else {
                    buf.line_to(screen_x, screen_y);
                }
            }
        }

        glyph.advance_width() * scale
    }

    /// Append the strokes of a text string into a vector buffer.
    ///
    /// Returns the total horizontal advance width in pixels.
    pub fn draw_string(
        &self,
        text: &str,
        start_x: f32,
        start_y: f32,
        scale: f32,
        buf: &mut VectorBuffer,
    ) -> f32 {
        let mut current_x = start_x;
        for ch in text.chars() {
            if ch == '\n' {
                // Newlines handled by caller or line advancement
                continue;
            }
            let advance = self.draw_char(ch, current_x, start_y, scale, buf);
            current_x += advance;
        }
        current_x - start_x
    }

    /// Measure the width and height of a text string in pixels at the given scale.
    pub fn measure_string(&self, text: &str, scale: f32) -> (f32, f32) {
        let mut width = 0.0f32;
        for ch in text.chars() {
            if let Some(glyph) = self.get_glyph(ch) {
                width += glyph.advance_width() * scale;
            } else {
                width += 16.0 * scale;
            }
        }
        // Hershey standard vertical range is roughly -12 to +14 = 26 units
        let height = 26.0 * scale;
        (width, height)
    }
}

/// Parse a single JHF glyph record.
fn parse_jhf_glyph(raw: &str) -> Option<HersheyGlyph> {
    if raw.len() < 10 {
        return None;
    }

    let glyph_id = raw[..5].trim().parse::<u32>().ok()?;
    let pair_count = raw[5..8].trim().parse::<usize>().ok()?;

    let bytes = raw.as_bytes();
    let left = (bytes[8] as i8).wrapping_sub(b'R' as i8);
    let right = (bytes[9] as i8).wrapping_sub(b'R' as i8);

    let coord_bytes = &bytes[10..];
    let mut strokes = Vec::new();
    let mut current_stroke = Vec::new();

    let mut i = 0;
    let max_pairs = pair_count.saturating_sub(1); // First pair was left/right margin

    while i + 1 < coord_bytes.len() && strokes.len() + (if current_stroke.is_empty() { 0 } else { 1 }) <= max_pairs + 10 {
        let cx = coord_bytes[i];
        let cy = coord_bytes[i + 1];
        i += 2;

        if cx == b' ' && cy == b'R' {
            // Pen up sentinel
            if !current_stroke.is_empty() {
                strokes.push(StrokeSegment { points: current_stroke });
                current_stroke = Vec::new();
            }
        } else {
            let px = (cx as i8).wrapping_sub(b'R' as i8);
            let py = (cy as i8).wrapping_sub(b'R' as i8);
            current_stroke.push((px, py));
        }
    }

    if !current_stroke.is_empty() {
        strokes.push(StrokeSegment { points: current_stroke });
    }

    Some(HersheyGlyph {
        glyph_id,
        left,
        right,
        strokes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hershey_simplex_loads_all_ascii() {
        let font = HersheyFont::load_simplex();

        // Printable ASCII 32..=126
        for ascii in 32u8..=126u8 {
            let ch = ascii as char;
            let glyph = font.get_glyph(ch);
            assert!(
                glyph.is_some(),
                "Glyph for ASCII '{}' ({}) should be loaded",
                ch,
                ascii
            );
            let g = glyph.unwrap();
            assert!(
                g.advance_width() > 0.0,
                "Glyph '{}' advance width should be > 0",
                ch
            );
        }

        // Test specific known characters
        let space = font.get_glyph(' ').unwrap();
        assert_eq!(space.glyph_id, 699);
        assert!(space.strokes.is_empty()); // Space has no visible strokes

        let letter_a = font.get_glyph('A').unwrap();
        assert_eq!(letter_a.glyph_id, 501);
        assert!(!letter_a.strokes.is_empty());
    }

    #[test]
    fn test_hershey_string_rendering() {
        let font = HersheyFont::load_simplex();
        let mut buf = VectorBuffer::new();

        let width = font.draw_string("Lotus 5-7-11", 0.0, 0.0, 1.0, &mut buf);
        assert!(width > 50.0, "Rendered text width should be positive and realistic");
        assert!(!buf.is_empty(), "Vector buffer should contain strokes");

        let (mw, mh) = font.measure_string("Lotus 5-7-11", 1.0);
        assert!((mw - width).abs() < 1.0, "Measured width should match drawn width");
        assert!(mh > 10.0, "Measured height should be positive");
    }
}
