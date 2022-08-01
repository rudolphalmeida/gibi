use clap::Parser;
use pixels::{Error, Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

use gibi::{
    gameboy::Gameboy,
    ppu::{LCD_HEIGHT, LCD_WIDTH},
};

use crate::gui::Framework;
use crate::options::Options;

mod gui;
mod options;

fn main() -> Result<(), Error> {
    env_logger::init();

    let options = Options::parse();
    let rom = std::fs::read(options.rom_file.as_str()).unwrap();
    log::info!("Loaded ROM file: {:?}", options.rom_file);

    // Initialize GUI, and pixel buffers
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(
            (LCD_WIDTH * options.scale_factor) as f64,
            (LCD_HEIGHT * options.scale_factor) as f64,
        );
        WindowBuilder::new()
            .with_title(format!("GiBi - {}", options.rom_file))
            .with_inner_size(size)
            .with_resizable(false)
            .build(&event_loop)
            .unwrap()
    };

    let (mut pixels, mut framework) = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        let pixels = Pixels::new(LCD_WIDTH, LCD_HEIGHT, surface_texture)?;
        let framework =
            Framework::new(window_size.width, window_size.height, scale_factor, &pixels);

        (pixels, framework)
    };

    let mut gameboy = Gameboy::new(rom);

    event_loop.run(move |event, _, control_flow| {
        gameboy.run_one_frame();
        gameboy.copy_framebuffer_to_draw_target(pixels.get_frame());

        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Update scale factor
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                pixels.resize_surface(size.width, size.height);
                framework.resize(size.width, size.height);
            }

            // TODO: Check for Joypad presses here
        }

        window.request_redraw();

        match event {
            Event::WindowEvent { event, .. } => {
                // Update egui inputs
                framework.handle_event(&event);
            }
            // Draw the current frame
            Event::RedrawRequested(_) => {
                // TODO: Draw the frame here

                framework.prepare(&window);

                // Render everything together
                let render_result = pixels.render_with(|encoder, render_target, context| {
                    // Render the world texture
                    context.scaling_renderer.render(encoder, render_target);
                    // Render egui
                    framework.render(encoder, render_target, context);

                    Ok(())
                });

                // Basic error handling
                if render_result
                    .map_err(|e| log::error!("pixels.render() failed: {:?}", e))
                    .is_err()
                {
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
}
