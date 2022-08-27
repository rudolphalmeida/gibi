use clap::Parser;
use pixels::{Error, Pixels, SurfaceTexture};
use std::{thread, time};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

use gibi::joypad::JoypadKeys;
use gibi::{
    gameboy::Gameboy,
    ppu::{LCD_HEIGHT, LCD_WIDTH},
};

use crate::options::Options;

mod options;

const JOYPAD_KEY_MAP: [(JoypadKeys, VirtualKeyCode); 8] = [
    (JoypadKeys::Right, VirtualKeyCode::Right),
    (JoypadKeys::Left, VirtualKeyCode::Left),
    (JoypadKeys::Up, VirtualKeyCode::Up),
    (JoypadKeys::Down, VirtualKeyCode::Down),
    (JoypadKeys::A, VirtualKeyCode::Z),
    (JoypadKeys::B, VirtualKeyCode::X),
    (JoypadKeys::Select, VirtualKeyCode::N),
    (JoypadKeys::Start, VirtualKeyCode::M),
];
const TARGET_FPS: u64 = 60;

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

    let mut pixels = {
        let window_size = window.inner_size();
        let _scale_factor = window.scale_factor() as f32;
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);

        Pixels::new(LCD_WIDTH, LCD_HEIGHT, surface_texture)?
    };

    let mut gameboy = Gameboy::new(rom, None);

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
            if let Some(_scale_factor) = input.scale_factor() {}

            // Resize the window
            if let Some(size) = input.window_resized() {
                pixels.resize_surface(size.width, size.height);
            }

            // TODO: Check for Joypad presses here
            for (joypad_key, keyboard_key) in JOYPAD_KEY_MAP {
                if input.key_pressed(keyboard_key) {
                    gameboy.keydown(joypad_key);
                } else if input.key_released(keyboard_key) {
                    gameboy.keyup(joypad_key);
                }
            }
        }

        window.request_redraw();

        match event {
            Event::WindowEvent { event: _, .. } => {}
            // Draw the current frame
            Event::RedrawRequested(_) => {
                // Render everything together
                let render_result = pixels.render_with(|encoder, render_target, context| {
                    // Render the world texture
                    context.scaling_renderer.render(encoder, render_target);

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
