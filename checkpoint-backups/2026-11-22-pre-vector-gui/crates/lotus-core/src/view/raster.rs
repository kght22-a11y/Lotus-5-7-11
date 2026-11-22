//! Software subpixel anti-aliased rasterizer and bitmap framebuffer surface.
//!
//! Implements ClearType-style subpixel horizontal anti-aliasing with FIR energy filtering
//! for razor-sharp rendering of Hershey stroke letterforms and vector paths directly onto
//! linear 32-bit pixel surfaces (`&mut [u32]`) without GPU or external C dependencies.

#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec};

use crate::model::vector::{ColorRgba, Point2D, VectorBuffer, VectorOp};

/// 2D affine transform matrix for panning and zooming.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
}

impl Transform2D {
    pub const fn identity() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }

    pub fn new(pan_x: f32, pan_y: f32, zoom: f32) -> Self {
        Self { pan_x, pan_y, zoom: zoom.max(0.01) }
    }

    /// Map a world point to screen coordinates.
    pub fn transform_point(&self, pt: Point2D) -> Point2D {
        Point2D::new(
            (pt.x - self.pan_x) * self.zoom,
            (pt.y - self.pan_y) * self.zoom,
        )
    }
}

/// A 32-bit pixel bitmap surface (`0x00RRGGBB`).
#[derive(Debug, Clone, PartialEq)]
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub pixels: Vec<u32>,
}

impl Bitmap {
    /// Create a new bitmap buffer initialized to black (0x00000000).
    pub fn new(width: usize, height: usize) -> Self {
        let size = width * height;
        Self {
            width,
            height,
            pitch: width,
            pixels: vec![0u32; size],
        }
    }

    /// Clear the entire bitmap with a solid 0x00RRGGBB color.
    pub fn clear(&mut self, color: u32) {
        self.pixels.fill(color);
    }

    /// Set a pixel with bounds checking.
    pub fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            self.pixels[y * self.pitch + x] = color;
        }
    }

    /// Get a pixel with bounds checking.
    pub fn get_pixel(&self, x: usize, y: usize) -> Option<u32> {
        if x < self.width && y < self.height {
            Some(self.pixels[y * self.pitch + x])
        } else {
            None
        }
    }

    /// Export the bitmap to an uncompressed 24-bit BMP image in memory.
    pub fn to_bmp_bytes(&self) -> Vec<u8> {
        let width = self.width as u32;
        let height = self.height as u32;
        let row_padding = (4 - ((width * 3) % 4)) % 4;
        let image_size = (width * 3 + row_padding) * height;
        let file_size = 54 + image_size;

        let mut bmp = Vec::with_capacity(file_size as usize);

        // --- BMP Header (14 bytes) ---
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&file_size.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes()); // Reserved
        bmp.extend_from_slice(&54u32.to_le_bytes()); // Offset to pixel array

        // --- DIB Header (BITMAPINFOHEADER - 40 bytes) ---
        bmp.extend_from_slice(&40u32.to_le_bytes()); // DIB Header size
        bmp.extend_from_slice(&(width as i32).to_le_bytes());
        bmp.extend_from_slice(&(height as i32).to_le_bytes()); // Bottom-up bitmap
        bmp.extend_from_slice(&1u16.to_le_bytes()); // 1 color plane
        bmp.extend_from_slice(&24u16.to_le_bytes()); // 24 bits per pixel (BGR)
        bmp.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB (uncompressed)
        bmp.extend_from_slice(&image_size.to_le_bytes());
        bmp.extend_from_slice(&2835u32.to_le_bytes()); // Horizontal resolution (72 DPI)
        bmp.extend_from_slice(&2835u32.to_le_bytes()); // Vertical resolution (72 DPI)
        bmp.extend_from_slice(&0u32.to_le_bytes()); // Colors in palette
        bmp.extend_from_slice(&0u32.to_le_bytes()); // Important colors

        // --- Pixel Data (Bottom-Up BGR) ---
        for y in (0..self.height).rev() {
            let row_offset = y * self.pitch;
            for x in 0..self.width {
                let pixel = self.pixels[row_offset + x];
                let r = ((pixel >> 16) & 0xFF) as u8;
                let g = ((pixel >> 8) & 0xFF) as u8;
                let b = (pixel & 0xFF) as u8;
                bmp.push(b);
                bmp.push(g);
                bmp.push(r);
            }
            for _ in 0..row_padding {
                bmp.push(0);
            }
        }

        bmp
    }
}

/// Software rasterizer implementing subpixel anti-aliased stroke rendering.
pub struct SubpixelRasterizer;

impl SubpixelRasterizer {
    /// Render a vector buffer onto a target bitmap with full subpixel ClearType filtering.
    pub fn render(
        bitmap: &mut Bitmap,
        vector_buf: &VectorBuffer,
        transform: &Transform2D,
    ) {
        if bitmap.width == 0 || bitmap.height == 0 || vector_buf.is_empty() {
            return;
        }

        let mut cur_pt = Point2D::new(0.0, 0.0);
        let mut poly_start = Point2D::new(0.0, 0.0);
        let mut cur_color = ColorRgba::WHITE;
        let mut cur_width = 1.0f32;

        for op in &vector_buf.ops {
            match op {
                VectorOp::MoveTo(p) => {
                    cur_pt = transform.transform_point(*p);
                    poly_start = cur_pt;
                }
                VectorOp::LineTo(p) => {
                    let next_pt = transform.transform_point(*p);
                    Self::draw_subpixel_line(bitmap, cur_pt, next_pt, cur_width * transform.zoom, cur_color);
                    cur_pt = next_pt;
                }
                VectorOp::SetColor(c) => {
                    cur_color = *c;
                }
                VectorOp::SetWidth(w) => {
                    cur_width = *w;
                }
                VectorOp::ClosePoly => {
                    Self::draw_subpixel_line(bitmap, cur_pt, poly_start, cur_width * transform.zoom, cur_color);
                    cur_pt = poly_start;
                }
            }
        }
    }

    /// Draw a line with 3x horizontal subpixel anti-aliasing and FIR energy filtering.
    pub fn draw_subpixel_line(
        bitmap: &mut Bitmap,
        p0: Point2D,
        p1: Point2D,
        width: f32,
        color: ColorRgba,
    ) {
        let width = width.max(0.75);
        let radius = width * 0.5;

        // Subpixel horizontal coordinate space: X is multiplied by 3
        let sub_width = (bitmap.width * 3) as i32;
        let bmp_height = bitmap.height as i32;

        let p0_sub = Point2D::new(p0.x * 3.0, p0.y);
        let p1_sub = Point2D::new(p1.x * 3.0, p1.y);
        let sub_radius_x = radius * 3.0;

        // Bounding box in subpixel coordinates
        let min_sub_x = ((p0_sub.x.min(p1_sub.x) - sub_radius_x - 2.0).floor() as i32).max(0);
        let max_sub_x = ((p0_sub.x.max(p1_sub.x) + sub_radius_x + 2.0).ceil() as i32).min(sub_width - 1);
        let min_y = ((p0_sub.y.min(p1_sub.y) - radius - 1.0).floor() as i32).max(0);
        let max_y = ((p0_sub.y.max(p1_sub.y) + radius + 1.0).ceil() as i32).min(bmp_height - 1);

        if min_sub_x > max_sub_x || min_y > max_y {
            return;
        }

        let dx = p1_sub.x - p0_sub.x;
        let dy = p1_sub.y - p0_sub.y;
        let len_sq = dx * dx + dy * dy;

        // Buffer for one scanline of subpixel coverages
        let sub_span_len = (max_sub_x - min_sub_x + 3) as usize;
        let mut sub_coverage = vec![0u8; sub_span_len];

        for y_idx in min_y..=max_y {
            let py = y_idx as f32 + 0.5;
            sub_coverage.fill(0);

            for sub_x_idx in min_sub_x..=max_sub_x {
                let px = sub_x_idx as f32 + 0.5;

                // Distance from point (px, py) to segment (p0_sub, p1_sub) in normalized metric
                let dist = if len_sq < 0.0001 {
                    let ddx = (px - p0_sub.x) / 3.0;
                    let ddy = py - p0_sub.y;
                    (ddx * ddx + ddy * ddy).sqrt()
                } else {
                    let t = (((px - p0_sub.x) * dx + (py - p0_sub.y) * dy) / len_sq).clamp(0.0, 1.0);
                    let proj_x = p0_sub.x + t * dx;
                    let proj_y = p0_sub.y + t * dy;
                    let ddx = (px - proj_x) / 3.0;
                    let ddy = py - proj_y;
                    (ddx * ddx + ddy * ddy).sqrt()
                };

                let d = dist - radius;
                let cov = if d <= -0.5 {
                    255u8
                } else if d >= 0.5 {
                    0u8
                } else {
                    ((0.5 - d) * 255.0).clamp(0.0, 255.0) as u8
                };

                sub_coverage[(sub_x_idx - min_sub_x) as usize] = cov;
            }

            // Apply ClearType FIR filter across subpixels [0.25, 0.50, 0.25]
            let start_pixel_x = min_sub_x / 3;
            let end_pixel_x = max_sub_x / 3;

            for px_idx in start_pixel_x..=end_pixel_x {
                if px_idx < 0 || px_idx >= bitmap.width as i32 {
                    continue;
                }

                let sub_base = (px_idx * 3) - min_sub_x;
                let get_cov = |offset: i32| -> u32 {
                    let idx = sub_base + offset;
                    if idx >= 0 && (idx as usize) < sub_coverage.len() {
                        sub_coverage[idx as usize] as u32
                    } else {
                        0
                    }
                };

                // FIR filter: R channel centers on 3X, G on 3X+1, B on 3X+2
                let r_cov = (get_cov(-1) + 2 * get_cov(0) + get_cov(1)) >> 2;
                let g_cov = (get_cov(0) + 2 * get_cov(1) + get_cov(2)) >> 2;
                let b_cov = (get_cov(1) + 2 * get_cov(2) + get_cov(3)) >> 2;

                if r_cov == 0 && g_cov == 0 && b_cov == 0 {
                    continue;
                }

                let dest_idx = (y_idx as usize) * bitmap.pitch + (px_idx as usize);
                let bg = bitmap.pixels[dest_idx];
                let bg_r = ((bg >> 16) & 0xFF) as u32;
                let bg_g = ((bg >> 8) & 0xFF) as u32;
                let bg_b = (bg & 0xFF) as u32;

                let fg_r = color.r as u32;
                let fg_g = color.g as u32;
                let fg_b = color.b as u32;

                // Alpha blend each channel with its subpixel coverage
                let out_r = (fg_r * r_cov + bg_r * (255 - r_cov)) / 255;
                let out_g = (fg_g * g_cov + bg_g * (255 - g_cov)) / 255;
                let out_b = (fg_b * b_cov + bg_b * (255 - b_cov)) / 255;

                bitmap.pixels[dest_idx] = (out_r << 16) | (out_g << 8) | out_b;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitmap_creation_and_bmp_export() {
        let mut bmp = Bitmap::new(64, 32);
        bmp.clear(0x00112233);
        assert_eq!(bmp.get_pixel(0, 0), Some(0x00112233));
        assert_eq!(bmp.get_pixel(63, 31), Some(0x00112233));
        assert_eq!(bmp.get_pixel(64, 32), None);

        let bmp_bytes = bmp.to_bmp_bytes();
        assert!(bmp_bytes.len() > 54);
        assert_eq!(&bmp_bytes[0..2], b"BM");
    }

    #[test]
    fn test_subpixel_line_rasterization() {
        let mut bmp = Bitmap::new(50, 50);
        bmp.clear(0x00000000); // Black background

        let mut buf = VectorBuffer::new();
        buf.set_color(ColorRgba::rgb(255, 255, 255)); // White line
        buf.set_width(1.0);
        buf.move_to(10.0, 10.0);
        buf.line_to(40.0, 10.0); // Horizontal line

        SubpixelRasterizer::render(&mut bmp, &buf, &Transform2D::identity());

        // Verify that pixels along the line at y=10 are illuminated
        let center_pixel = bmp.get_pixel(25, 10).unwrap();
        assert!(center_pixel > 0, "Pixel along line should be illuminated");

        // Verify that pixels far away are untouched
        let distant_pixel = bmp.get_pixel(25, 30).unwrap();
        assert_eq!(distant_pixel, 0, "Distant pixel should remain black");
    }
}
