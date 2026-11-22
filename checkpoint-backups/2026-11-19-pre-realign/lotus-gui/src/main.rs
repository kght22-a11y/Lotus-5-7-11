use lotus_core::model::{VectorBuffer, VectorOp, ColorRgba, Point2D};
use std::fs::File;
use std::io::Write;

/// The Projection Engine: Converts a VectorBuffer into an SVG string.
/// This is the "UI as a Document" implementation.
struct SvgProjector;

impl SvgProjector {
    fn project(buffer: &VectorBuffer) -> String {
        let mut svg = String::new();
        let mut current_color = "black";
        let mut current_width = "1.0";
        let mut paths = Vec::new();
        let mut current_path = String::new();

        for op in &buffer.ops {
            match op {
                VectorOp::MoveTo(p) => {
                    if !current_path.is_empty() {
                        paths.push(format!(r#"<path d="{} Z" stroke="{}" stroke-width="{}" fill="none" />"#, 
                            current_path, current_color, current_width));
                    }
                    current_path = format!("M {} {}", p.x, p.y);
                }
                VectorOp::LineTo(p) => {
                    current_path.push_str(&format!(" L {} {}", p.x, p.y));
                }
                VectorOp::SetColor(c) => {
                    current_color = format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a / 255.0);
                }
                VectorOp::SetWidth(w) => {
                    current_width = w.to_string();
                }
                VectorOp::ClosePoly => {
                    if !current_path.is_empty() {
                        paths.push(format!(r#"<path d="{} Z" stroke="{}" stroke-width="{}" fill="none" />"#, 
                            current_path, current_color, current_width));
                        current_path = String::new();
                    }
                }
            }
        }

        if !current_path.is_empty() {
            paths.push(format!(r#"<path d="{} Z" stroke="{}" stroke-width="{}" fill="none" />"#, 
                current_path, current_color, current_width));
        }

        let bounds = buffer.bounds().unwrap_or(lotus_core::model::Rect2D::new(0.0, 0.0, 800.0, 600.0));
        
        format!(
            r#"<svg width="{}" height="{}" viewBox="{} {} {} {}" xmlns="http://www.w3.org/2000/svg">
            <rect width="100%" height="100%" fill="#c0c0c0" />
            {}
            </svg>"#,
            bounds.width(),
            bounds.height(),
            bounds.min_x,
            bounds.min_y,
            bounds.max_x,
            bounds.max_y,
            paths.join("\n")
        )
    }
}

/// GeoWorks Ensemble Primitives
struct GeoWorks;

impl GeoWorks {
    /// Renders a classic beveled window.
    fn window(x: f32, y: f32, w: f32, h: f32, title: &str) -> VectorBuffer {
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
        // We simulate a fill with a series of close lines or just a box for now
        buf.rect(x + 2.0, y + 2.0, w - 4.0, 20.0);

        // Title Text (Represented as a simple line for this beta)
        buf.set_color(ColorRgba::WHITE);
        buf.move_to(x + 10.0, y + 12.0);
        buf.line_to(x + 10.0 + (title.len() as f32 * 8.0), y + 12.0);

        buf
    }
}

fn main() {
    println!("Lotus Beta Vector Interface");
    
    // Mock a workbook state: A Window at 50,50
    let mut scene = VectorBuffer::new();
    let win = GeoWorks::window(50.0, 50.0, 300.0, 200.0, "Sovereign Workspace");
    
    // Merge window into scene
    for op in win.ops {
        scene.ops.push(op);
    }

    // Add some "vector state machine" elements
    scene.set_color(ColorRgba::RED);
    scene.set_width(1.0);
    scene.move_to(100.0, 100.0);
    scene.line_to(150.0, 150.0);
    scene.line_to(200.0, 100.0);
    scene.close_poly();

    let svg_content = SvgProjector::project(&scene);
    
    let mut file = File::create("lotus_beta_ui.svg").expect("Unable to create file");
    file.write_all(svg_content.as_bytes()).expect("Unable to write data");
    
    println!("Projection complete. Open 'lotus_beta_ui.svg' to view the interface.");
}
