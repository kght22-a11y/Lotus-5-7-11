//! Logo-style Turtle Graphics engine for embedded Modern BASIC.
//!
//! Provides coordinate tracking, relative turtle movement, angle turns,
//! pen attributes, Hershey text rendering along turtle trajectories,
//! and vector command buffer generation.

#[cfg(not(feature = "std"))]
use alloc::string::String;

use crate::model::vector::{ColorRgba, VectorBuffer};
use crate::view::hershey::HersheyFont;

/// A Logo turtle state machine that accumulates vector path commands.
#[derive(Debug, Clone, PartialEq)]
pub struct Turtle {
    pub x: f32,
    pub y: f32,
    /// Heading in degrees.
    /// 0° = North / UP (-Y in screen space)
    /// 90° = East / RIGHT (+X in screen space)
    /// 180° = South / DOWN (+Y in screen space)
    /// 270° = West / LEFT (-X in screen space)
    pub heading_deg: f32,
    pub pen_down: bool,
    pub color: ColorRgba,
    pub width: f32,
    pub buffer: VectorBuffer,
}

impl Default for Turtle {
    fn default() -> Self {
        Self::new()
    }
}

impl Turtle {
    pub fn new() -> Self {
        let mut buffer = VectorBuffer::new();
        buffer.set_color(ColorRgba::WHITE);
        buffer.set_width(1.0);
        buffer.move_to(0.0, 0.0);

        Self {
            x: 0.0,
            y: 0.0,
            heading_deg: 0.0,
            pen_down: true,
            color: ColorRgba::WHITE,
            width: 1.0,
            buffer,
        }
    }

    /// Reset turtle to origin (0, 0) facing North (0°).
    pub fn home(&mut self) {
        self.x = 0.0;
        self.y = 0.0;
        self.heading_deg = 0.0;
        if self.pen_down {
            self.buffer.line_to(0.0, 0.0);
        } else {
            self.buffer.move_to(0.0, 0.0);
        }
    }

    /// Clear all accumulated vector commands and reset turtle state.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.buffer.set_color(self.color);
        self.buffer.set_width(self.width);
        self.home();
    }

    /// Move turtle forward by `dist` units along its current heading.
    pub fn forward(&mut self, dist: f32) {
        let rad = self.heading_deg.to_radians();
        let sin = rad.sin();
        let cos = rad.cos();

        // 0° is North (screen -Y), 90° is East (screen +X)
        let next_x = self.x + dist * sin;
        let next_y = self.y - dist * cos;

        if self.pen_down {
            self.buffer.line_to(next_x, next_y);
        } else {
            self.buffer.move_to(next_x, next_y);
        }

        self.x = next_x;
        self.y = next_y;
    }

    /// Move turtle backward by `dist` units along its current heading.
    pub fn back(&mut self, dist: f32) {
        self.forward(-dist);
    }

    /// Turn right (clockwise) by `angle_deg` degrees.
    pub fn right(&mut self, angle_deg: f32) {
        self.heading_deg = normalize_degrees(self.heading_deg + angle_deg);
    }

    /// Turn left (counter-clockwise) by `angle_deg` degrees.
    pub fn left(&mut self, angle_deg: f32) {
        self.heading_deg = normalize_degrees(self.heading_deg - angle_deg);
    }

    /// Set pen to down state (subsequent movements draw lines).
    pub fn pen_up(&mut self) {
        self.pen_down = false;
    }

    /// Set pen to down state (subsequent movements draw lines).
    pub fn pen_down(&mut self) {
        self.pen_down = true;
    }

    /// Set stroke color.
    pub fn set_color(&mut self, color: ColorRgba) {
        self.color = color;
        self.buffer.set_color(color);
    }

    /// Set stroke width in points/pixels.
    pub fn set_width(&mut self, width: f32) {
        let width = width.max(0.25);
        self.width = width;
        self.buffer.set_width(width);
    }

    /// Jump to absolute coordinate (x, y).
    pub fn set_pos(&mut self, x: f32, y: f32) {
        if self.pen_down {
            self.buffer.line_to(x, y);
        } else {
            self.buffer.move_to(x, y);
        }
        self.x = x;
        self.y = y;
    }

    /// Set absolute heading in degrees (0° = North, 90° = East, etc.).
    pub fn set_heading(&mut self, deg: f32) {
        self.heading_deg = normalize_degrees(deg);
    }

    /// Render Hershey stroke text oriented along the turtle's current trajectory.
    ///
    /// If heading is 90° (East), text is oriented upright and reads left-to-right.
    /// Advances the turtle along its heading by the measured string advance width.
    pub fn label(&mut self, text: &str) {
        let font = HersheyFont::load_simplex();
        let scale = (self.width * 0.75).clamp(0.5, 5.0);

        // Heading 90° is standard reading left-to-right
        let rotation_rad = (self.heading_deg - 90.0).to_radians();
        let cos_r = rotation_rad.cos();
        let sin_r = rotation_rad.sin();

        let mut pen_advance = 0.0f32;

        for ch in text.chars() {
            if ch == '\n' {
                continue;
            }
            if let Some(glyph) = font.get_glyph(ch) {
                let left_offset = glyph.left as f32;

                for stroke in &glyph.strokes {
                    let mut is_first = true;
                    for &(px, py) in &stroke.points {
                        let gx = (px as f32 - left_offset) * scale + pen_advance;
                        let gy = (py as f32) * scale;

                        // Rotate by turtle orientation
                        let rot_x = gx * cos_r - gy * sin_r;
                        let rot_y = gx * sin_r + gy * cos_r;

                        let screen_x = self.x + rot_x;
                        let screen_y = self.y + rot_y;

                        if is_first {
                            self.buffer.move_to(screen_x, screen_y);
                            is_first = false;
                        } else {
                            self.buffer.line_to(screen_x, screen_y);
                        }
                    }
                }

                let advance = glyph.advance_width() * scale;
                pen_advance += advance;
            } else {
                pen_advance += 16.0 * scale;
            }
        }

        // Advance turtle to the end of the text label
        self.forward(pen_advance);
    }
}

/// Normalize an angle in degrees into [0.0, 360.0).
fn normalize_degrees(deg: f32) -> f32 {
    let rem = deg % 360.0;
    if rem < 0.0 {
        rem + 360.0
    } else {
        rem
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turtle_square() {
        let mut turtle = Turtle::new();

        // Draw a 100x100 square
        for _ in 0..4 {
            turtle.forward(100.0);
            turtle.right(90.0);
        }

        // Should return close to origin
        assert!(turtle.x.abs() < 0.001);
        assert!(turtle.y.abs() < 0.001);
        assert!((turtle.heading_deg - 0.0).abs() < 0.001);

        // Initial move_to + 4 line_to = 5 operations
        assert_eq!(turtle.buffer.len(), 7); // SetColor, SetWidth, MoveTo(0,0) + 4 LineTo
    }

    #[test]
    fn test_turtle_label() {
        let mut turtle = Turtle::new();
        turtle.set_heading(90.0); // Facing East
        turtle.label("LOTUS");

        assert!(turtle.x > 30.0, "Turtle should have advanced horizontally");
        assert!(turtle.buffer.len() > 10, "Buffer should contain Hershey text strokes");
    }
}
