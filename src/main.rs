use clap::Parser;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::render::{TextureAccess, TextureCreator};
use sdl2::video::WindowContext;

use gibi::{
    gameboy::Gameboy,
    ppu::{LCD_HEIGHT, LCD_WIDTH},
};
use spin_sleep::LoopHelper;

use crate::options::Options;

mod options;

// const _JOYPAD_KEY_MAP: [(JoypadKeys, VirtualKeyCode); 8] = [
//     (JoypadKeys::Right, VirtualKeyCode::Right),
//     (JoypadKeys::Left, VirtualKeyCode::Left),
//     (JoypadKeys::Up, VirtualKeyCode::Up),
//     (JoypadKeys::Down, VirtualKeyCode::Down),
//     (JoypadKeys::A, VirtualKeyCode::Z),
//     (JoypadKeys::B, VirtualKeyCode::X),
//     (JoypadKeys::Select, VirtualKeyCode::N),
//     (JoypadKeys::Start, VirtualKeyCode::M),
// ];
const TARGET_FPS: f32 = 60.0;

fn main() {
    env_logger::init();

    let options = Options::parse();
    let rom = std::fs::read(options.rom_file.as_str()).unwrap();
    log::info!("Loaded ROM file: {:?}", options.rom_file);

    // Initialize GUI, and pixel buffers
    let sdl = sdl2::init().unwrap();
    let video_subsystem = sdl.video().unwrap();

    let mut window_builder = video_subsystem.window(
        &format!("GiBi - {}", options.rom_file),
        LCD_WIDTH * options.scale_factor,
        LCD_HEIGHT * options.scale_factor,
    );
    let window = window_builder
        .allow_highdpi()
        .position_centered()
        .vulkan()
        .build()
        .unwrap();
    let mut renderer = window.into_canvas().accelerated().build().unwrap();
    renderer.set_draw_color(Color::RGBA(0x00, 0x00, 0x00, 0xFF));
    let texture_creator = renderer.texture_creator();
    let texture_creator_pointer = &texture_creator as *const TextureCreator<WindowContext>;
    let mut texture = unsafe { &*texture_creator_pointer }
        .create_texture(
            PixelFormatEnum::RGBA32,
            TextureAccess::Streaming,
            LCD_WIDTH,
            LCD_HEIGHT,
        )
        .unwrap();

    let mut event_pump = sdl.event_pump().unwrap();
    let mut loop_helper = LoopHelper::builder()
        .report_interval_s(0.5) // report every half a second
        .build_with_target_rate(TARGET_FPS); // limit to 60.0 FPS if possible

    let mut gameboy = Gameboy::new(rom, None);
    let mut pixels = vec![0x00; LCD_WIDTH as usize * LCD_HEIGHT as usize * 4];

    'mainloop: loop {
        loop_helper.loop_start();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'mainloop,
                _ => {}
            }
        }

        gameboy.run_one_frame();
        gameboy.copy_framebuffer_to_draw_target(&mut pixels);

        renderer.clear();
        texture
            .update(None, &pixels, LCD_WIDTH as usize * 4)
            .unwrap();
        renderer.copy(&texture, None, None).unwrap();
        renderer.present();

        loop_helper.loop_sleep();
    }
}
