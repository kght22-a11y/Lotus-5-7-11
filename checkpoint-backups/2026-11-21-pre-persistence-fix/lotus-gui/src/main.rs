use lotus_core::model::{VectorBuffer};
use lotus_core::view::{Bitmap, SubpixelRasterizer, Transform2D, HersheyFont, gui::GeoWorks};
use std::fs::File;
use std::io::Write;

fn main() {
    println!("Lotus Beta Vector Interface (Rasterizer Path)");
    
    let font = HersheyFont::load_simplex();
    
    // 1. Build the Scene as a VectorBuffer
    let mut scene = VectorBuffer::new();
    
    // Draw a GeoWorks window
    let win = GeoWorks::window(50.0, 50.0, 300.0, 200.0, "Sovereign Workspace", &font);
    for op in win.ops {
        scene.ops.push(op);
    }

    // Add some arbitrary vector state
    scene.set_color(lotus_core::model::ColorRgba::RED);
    scene.set_width(2.0);
    scene.move_to(100.0, 100.0);
    scene.line_to(150.0, 150.0);
    scene.line_to(200.0, 100.0);
    scene.close_poly();

    // 2. Initialize the Bitmap (The "Document" Buffer)
    let mut bitmap = Bitmap::new(800, 600);
    bitmap.clear(0xCCCCCC00); // Light gray background (GeoWorks style)

    // 3. Render using the SubpixelRasterizer
    SubpixelRasterizer::render(&mut bitmap, &scene, &Transform2D::identity());

    // 4. Export to BMP
    let bmp_bytes = bitmap.to_bmp_bytes();
    let mut file = File::create("lotus_beta_ui.bmp").expect("Unable to create file");
    file.write_all(&bmp_bytes).expect("Unable to write data");
    
    println!("Projection complete. Open 'lotus_beta_ui.bmp' to view the interface.");
}
