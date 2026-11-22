use lotus_core::model::{VectorBuffer, Workbook, Sheet, Cell, Coord};
use lotus_core::view::{Bitmap, SubpixelRasterizer, Transform2D, HersheyFont, gui::{GeoWorks, Button}};
use winit::{
    event::{Event, WindowEvent, ElementState, MouseButton},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;

fn main() {
    println!("Lotus Beta Vector Interface - Interactive");

    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("Lotus 5-7-11 Sovereign Workspace")
        .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0))
        .build(&event_loop)
        .unwrap();

    let context = Context::new(&window).unwrap();
    let mut surface = Surface::new(&context, &window).unwrap();

    let font = HersheyFont::load_simplex();
    
    // State: A simple workbook and a list of interactive buttons
    let mut wb = Workbook::new("wb", "Sovereign Workspace");
    let mut sheet = Sheet::new("s1", "Main");
    sheet.set_cell(Cell::text(Coord::new(0, 0), "Ready").with_name("status"));
    wb.sheets.push(sheet);

    let mut buttons = Vec::new();
    let mut scene = VectorBuffer::new();
    
    // Main Window
    let win = GeoWorks::window(50.0, 50.0, 300.0, 200.0, "Sovereign Workspace", &font);
    scene.ops.extend(win.ops);

    // Add an interactive button
    let btn = GeoWorks::button(70.0, 80.0, 120.0, 30.0, "Click Me", &font, Coord::new(0, 0));
    scene.ops.extend(btn.ops.clone());
    buttons.push(btn);

    let mut bitmap = Bitmap::new(800, 600);
    bitmap.clear(0xCCCCCC00);

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::MouseInput { state: ElementState::Pressed(MouseButton::Left), .. } => {
                    // In a real app, we'd use the current cursor position
                    // For this POC, we'll just trigger the first button
                    if let Some(btn) = buttons.first() {
                        println!("Button clicked! Mutating cell {:?}", btn.cell_ref);
                        if let Some(sheet) = wb.sheets.get_mut(0) {
                            sheet.set_cell(Cell::text(btn.cell_ref, "Mutated!"));
                        }
                        // Re-render scene with updated text? 
                        // (In a full impl, we'd regenerate the scene based on workbook state)
                        println!("Status cell updated to: Mutated!");
                    }
                }
                WindowEvent::RedrawRequested => {
                    let (width, height) = {
                        let size = window.inner_size();
                        (size.width, size.height)
                    };

                    surface.resize(
                        NonZeroU32::new(width).unwrap(),
                        NonZeroU32::new(height).unwrap(),
                    ).unwrap();

                    let mut buffer = surface.buffer_mut().unwrap();
                    
                    // Render the scene to the bitmap
                    bitmap.clear(0xCCCCCC00);
                    SubpixelRasterizer::render(&mut bitmap, &scene, &Transform2D::identity());
                    
                    for (i, pixel) in bitmap.pixels.iter().enumerate() {
                        if i < buffer.len() {
                            buffer[i] = *pixel;
                        }
                    }

                    buffer.present().unwrap();
                }
                _ => {}
            },
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    }).unwrap();
}
