//! Vector graphics primitives, command stream representation, and binary serialization.

#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec};

/// A 2D point in continuous vector coordinate space.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Point2D {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A 2D bounding rectangle in vector space.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect2D {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl Rect2D {
    pub fn new(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Self { min_x, min_y, max_x, max_y }
    }

    pub fn width(&self) -> f32 {
        self.max_x - self.min_x
    }

    pub fn height(&self) -> f32 {
        self.max_y - self.min_y
    }
}

/// 32-bit RGBA color representation.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorRgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ColorRgba {
    pub const BLACK: Self = Self::rgba(0, 0, 0, 255);
    pub const WHITE: Self = Self::rgba(255, 255, 255, 255);
    pub const RED: Self = Self::rgba(255, 0, 0, 255);
    pub const GREEN: Self = Self::rgba(0, 255, 0, 255);
    pub const BLUE: Self = Self::rgba(0, 0, 255, 255);
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Construct from a 0x00RRGGBB integer (with alpha = 255).
    pub const fn from_u32_rgb(val: u32) -> Self {
        Self {
            r: ((val >> 16) & 0xFF) as u8,
            g: ((val >> 8) & 0xFF) as u8,
            b: (val & 0xFF) as u8,
            a: 255,
        }
    }

    /// Pack into 0x00RRGGBB format.
    pub const fn to_u32_rgb(&self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    /// Pack into 0xAARRGGBB format.
    pub const fn to_u32_argb(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }
}

/// Stroke styling attributes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeStyle {
    pub width: f32,
    pub color: ColorRgba,
}

impl StrokeStyle {
    pub const fn new(width: f32, color: ColorRgba) -> Self {
        Self { width, color }
    }
}

/// A discrete vector path operation.
#[derive(Debug, Clone, PartialEq)]
pub enum VectorOp {
    MoveTo(Point2D),
    LineTo(Point2D),
    SetColor(ColorRgba),
    SetWidth(f32),
    ClosePoly,
}

/// An append-only buffer of vector operations.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VectorBuffer {
    pub ops: Vec<VectorOp>,
}

impl VectorBuffer {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self { ops: Vec::with_capacity(capacity) }
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn clear(&mut self) {
        self.ops.clear();
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        self.ops.push(VectorOp::MoveTo(Point2D::new(x, y)));
    }

    pub fn line_to(&mut self, x: f32, y: f32) {
        self.ops.push(VectorOp::LineTo(Point2D::new(x, y)));
    }

    pub fn set_color(&mut self, color: ColorRgba) {
        self.ops.push(VectorOp::SetColor(color));
    }

    pub fn set_width(&mut self, width: f32) {
        self.ops.push(VectorOp::SetWidth(width));
    }

    pub fn close_poly(&mut self) {
        self.ops.push(VectorOp::ClosePoly);
    }

    /// Append a rectangle path (unfilled outline).
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.move_to(x, y);
        self.line_to(x + w, y);
        self.line_to(x + w, y + h);
        self.line_to(x, y + h);
        self.close_poly();
    }

    /// Compute the bounding rectangle of all points in the buffer.
    pub fn bounds(&self) -> Option<Rect2D> {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let mut has_points = false;

        for op in &self.ops {
            let pt = match op {
                VectorOp::MoveTo(p) | VectorOp::LineTo(p) => Some(*p),
                _ => None,
            };
            if let Some(p) = pt {
                has_points = true;
                if p.x < min_x { min_x = p.x; }
                if p.y < min_y { min_y = p.y; }
                if p.x > max_x { max_x = p.x; }
                if p.y > max_y { max_y = p.y; }
            }
        }

        if has_points {
            Some(Rect2D::new(min_x, min_y, max_x, max_y))
        } else {
            None
        }
    }

    /// Serialize into a compact binary byte slice suitable for `CellValue::Blob`.
    ///
    /// Opcode Tags:
    /// - 0x01: MoveTo (f32, f32) [9 bytes]
    /// - 0x02: LineTo (f32, f32) [9 bytes]
    /// - 0x03: SetColor (u8, u8, u8, u8) [5 bytes]
    /// - 0x04: SetWidth (f32) [5 bytes]
    /// - 0x05: ClosePoly [1 byte]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        // Magic header: b"LVEC" (Lotus Vector) + version 1
        bytes.extend_from_slice(b"LVEC\x01");

        for op in &self.ops {
            match op {
                VectorOp::MoveTo(p) => {
                    bytes.push(0x01);
                    bytes.extend_from_slice(&p.x.to_le_bytes());
                    bytes.extend_from_slice(&p.y.to_le_bytes());
                }
                VectorOp::LineTo(p) => {
                    bytes.push(0x02);
                    bytes.extend_from_slice(&p.x.to_le_bytes());
                    bytes.extend_from_slice(&p.y.to_le_bytes());
                }
                VectorOp::SetColor(c) => {
                    bytes.push(0x03);
                    bytes.push(c.r);
                    bytes.push(c.g);
                    bytes.push(c.b);
                    bytes.push(c.a);
                }
                VectorOp::SetWidth(w) => {
                    bytes.push(0x04);
                    bytes.extend_from_slice(&w.to_le_bytes());
                }
                VectorOp::ClosePoly => {
                    bytes.push(0x05);
                }
            }
        }
        bytes
    }

    /// Deserialize a vector buffer from a compact binary byte slice.
    pub fn from_bytes(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 5 {
            return Err("Binary data too short for vector header");
        }
        if &data[0..4] != b"LVEC" {
            return Err("Invalid vector magic header");
        }
        if data[4] != 0x01 {
            return Err("Unsupported vector format version");
        }

        let mut ops = Vec::new();
        let mut idx = 5;

        while idx < data.len() {
            let tag = data[idx];
            idx += 1;
            match tag {
                0x01 => {
                    if idx + 8 > data.len() { return Err("Truncated MoveTo operation"); }
                    let x = f32::from_le_bytes(data[idx..idx + 4].try_into().unwrap());
                    let y = f32::from_le_bytes(data[idx + 4..idx + 8].try_into().unwrap());
                    idx += 8;
                    ops.push(VectorOp::MoveTo(Point2D::new(x, y)));
                }
                0x02 => {
                    if idx + 8 > data.len() { return Err("Truncated LineTo operation"); }
                    let x = f32::from_le_bytes(data[idx..idx + 4].try_into().unwrap());
                    let y = f32::from_le_bytes(data[idx + 4..idx + 8].try_into().unwrap());
                    idx += 8;
                    ops.push(VectorOp::LineTo(Point2D::new(x, y)));
                }
                0x03 => {
                    if idx + 4 > data.len() { return Err("Truncated SetColor operation"); }
                    let r = data[idx];
                    let g = data[idx + 1];
                    let b = data[idx + 2];
                    let a = data[idx + 3];
                    idx += 4;
                    ops.push(VectorOp::SetColor(ColorRgba::rgba(r, g, b, a)));
                }
                0x04 => {
                    if idx + 4 > data.len() { return Err("Truncated SetWidth operation"); }
                    let w = f32::from_le_bytes(data[idx..idx + 4].try_into().unwrap());
                    idx += 4;
                    ops.push(VectorOp::SetWidth(w));
                }
                0x05 => {
                    ops.push(VectorOp::ClosePoly);
                }
                _ => return Err("Unknown vector opcode tag"),
            }
        }

        Ok(Self { ops })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_buffer_roundtrip() {
        let mut buf = VectorBuffer::new();
        buf.set_color(ColorRgba::rgb(255, 128, 0));
        buf.set_width(2.5);
        buf.rect(10.0, 20.0, 100.0, 50.0);

        assert_eq!(buf.len(), 7); // SetColor + SetWidth + MoveTo + 3*LineTo + ClosePoly
        let bounds = buf.bounds().expect("bounds should exist");
        assert_eq!(bounds.min_x, 10.0);
        assert_eq!(bounds.min_y, 20.0);
        assert_eq!(bounds.max_x, 110.0);
        assert_eq!(bounds.max_y, 70.0);

        let bytes = buf.to_bytes();
        let decoded = VectorBuffer::from_bytes(&bytes).expect("deserialization should succeed");
        assert_eq!(buf, decoded);
    }
}
