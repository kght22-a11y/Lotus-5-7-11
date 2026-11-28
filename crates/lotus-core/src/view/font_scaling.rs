//! Font scaling utilities for vector-based UI elements.
//!
//! Provides functions to calculate appropriate text sizes based on available space,
//! content length, and rendering constraints.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};

use crate::view::HersheyFont;

/// Calculate the optimal font size for a given text to fit within specified dimensions.
///
/// # Arguments
/// * `text` - The text to render
/// * `max_width` - Maximum width available for rendering (in pixels)
/// * `max_height` - Maximum height available for rendering (in pixels)  
/// * `font` - Font to use for measurements
/// * `min_scale` - Minimum scaling factor allowed (default: 0.5)
/// * `max_scale` - Maximum scaling factor allowed (default: 2.0)
///
/// # Returns
/// The appropriate scale factor for the font to fit within constraints.
pub fn calculate_font_scale(
    text: &str,
    max_width: f32,
    max_height: f32,
    font: &HersheyFont,
    min_scale: f32,
    max_scale: f32,
) -> f32 {
    // If no content, return default scale
    if text.is_empty() {
        return 1.0;
    }
    
    let mut best_scale = 1.0;
    let mut current_scale = min_scale;
    
    // Binary search for the optimal scale within bounds  
    while current_scale <= max_scale {
        let (width, height) = font.measure_string(text, current_scale);
        
        if width <= max_width && height <= max_height {
            best_scale = current_scale;
            
            // Try a slightly larger scale to see if it still fits
            let next_scale = current_scale + 0.1;
            if next_scale > max_scale {
                break;
            }
            
            let (next_width, next_height) = font.measure_string(text, next_scale);
            if next_width <= max_width && next_height <= max_height {
                current_scale = next_scale;
            } else {
                // No larger scale fits - we found the optimal
                break;
            }
        } else {
            // Scale too large - try smaller
            let next_scale = current_scale - 0.1;
            if next_scale < min_scale {
                break;
            }
            current_scale = next_scale;
        }
    }
    
    best_scale
}

/// Calculate the dimensions needed for text rendering at a specific scale.
///
/// # Arguments
/// * `text` - The text to measure  
/// * `scale` - Scaling factor to apply to font size
/// * `font` - Font to use for measurements
///
/// # Returns
/// Tuple of (width, height) in pixels needed for rendering the text
pub fn calculate_text_dimensions(
    text: &str,
    scale: f32,
    font: &HersheyFont,
) -> (f32, f32) {
    if text.is_empty() {
        return (0.0, 0.0);
    }
    
    font.measure_string(text, scale)
}

/// Scale text to fit within a given rectangle while maintaining aspect ratio.
///
/// # Arguments
/// * `text` - Text to render
/// * `rect_width` - Width of target rectangle  
/// * `rect_height` - Height of target rectangle
/// * `font` - Font to use for measurements
///
/// # Returns
/// The optimal scale factor and the actual dimensions that will be used
pub fn fit_text_to_rectangle(
    text: &str,
    rect_width: f32,
    rect_height: f32,
    font: &HersheyFont,
) -> (f32, f32, f32) {
    // Start with default scale and calculate how much we can increase it
    let mut current_scale = 1.0;
    
    loop {
        let (width, height) = font.measure_string(text, current_scale);
        
        if width <= rect_width && height <= rect_height {
            // Fits - try to make it as large as possible while still fitting
            let next_scale = current_scale + 0.1;
            let (next_width, next_height) = font.measure_string(text, next_scale);
            
            if next_width <= rect_width && next_height <= rect_height {
                current_scale = next_scale; // Try larger
            } else {
                return (current_scale, width, height); // Found optimal size
            }
        } else {
            // Doesn't fit - need to make smaller 
            let next_scale = current_scale - 0.1;
            if next_scale <= 0.0 {
                return (0.05, rect_width.min(rect_height), rect_height.min(rect_width)); // Minimum scale fallback
            }
            current_scale = next_scale;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_font_scale_empty() {
        let font = HersheyFont::load_simplex();
        let scale = calculate_font_scale("", 100.0, 100.0, &font, 0.5, 2.0);
        assert_eq!(scale, 1.0);
    }

    #[test]
    fn test_calculate_text_dimensions() {
        let font = HersheyFont::load_simplex();
        let (width, height) = calculate_text_dimensions("Hello", 1.0, &font);
        assert!(width > 0.0);
        assert!(height > 0.0);
    }

    #[test]
    fn test_fit_text_to_rectangle() {
        let font = HersheyFont::load_simplex();
        let (scale, width, height) = fit_text_to_rectangle("Hello World", 200.0, 50.0, &font);
        assert!(scale > 0.0);
        assert!(width <= 200.0);
        assert!(height <= 50.0);
    }
}