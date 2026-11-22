//! High-level GUI primitives built on top of VectorBuffer.

use crate::model::{ColorRgba, VectorBuffer};

pub struct GeoWorks;

impl GeoWorks {
    /// Renders a classic beveled window.
    pub fn window(x: f32, y: f32, w: f32, h: f32, title: &str, font: &crate::view::HersheyFont) -> VectorBuffer {
        let mut buf = VectorBuffer::new();
        
        // Outer Border (Shadow/Light effect)
        buf.set_color(ColorRgba::rgb(0, 0, 0)); // Black shadow
        buf.set_width(2.0);
        buf.rect(x, y, w, h);

        buf.set_color(ColorRgba::rgb(255, 255, 255)); // White highlight
        buf.rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0);

        // Title Bar
        buf.set_color(ColorRgba::rgb(0, 0, 128)); // Classic Navy Blue
        buf.set_width(1.0);
        buf.rect(x + 2.0, y + 2.0, w - 4.0, 20.0);

        // Title Text
        buf.set_color(ColorRgba::WHITE);
        buf.set_width(1.0);
        font.draw_string(title, x + 10.0, y + 12.0, 1.0, &mut buf);

        buf
    }
}
