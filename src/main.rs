use std::collections::HashMap;
use std::path::Path;

use clap::Parser;

use gibi::joypad::JoypadKeys;
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

const TARGET_FPS: f32 = 60.0;

fn main() {
    env_logger::init();

    let options = Options::parse();
    let rom_file_path = Path::new(&options.rom_file);
    let save_file_path = rom_file_path.with_extension("sav");

    let rom = std::fs::read(rom_file_path).unwrap();
    log::info!("Loaded ROM file: {:?}", options.rom_file);

    let ram = if save_file_path.exists() {
        log::info!("Found a save file for ROM. Trying to load");
        std::fs::read(&save_file_path).ok()
    } else {
        log::info!("Did not find a save file for ROM");
        None
    };

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

    let joypad_keymap: HashMap<Keycode, JoypadKeys> = HashMap::from([
        (Keycode::Z, JoypadKeys::B),
        (Keycode::X, JoypadKeys::A),
        (Keycode::N, JoypadKeys::Select),
        (Keycode::M, JoypadKeys::Start),
        (Keycode::Down, JoypadKeys::Down),
        (Keycode::Up, JoypadKeys::Up),
        (Keycode::Left, JoypadKeys::Left),
        (Keycode::Right, JoypadKeys::Right),
    ]);

    let mut gameboy = Gameboy::new(rom, ram);
    let mut pixels = vec![0x00; LCD_WIDTH as usize * LCD_HEIGHT as usize * 4];

    'mainloop: loop {
        loop_helper.loop_start();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'mainloop,
                Event::KeyDown {
                    keycode: Some(x), ..
                } => match x {
                    Keycode::Escape => break 'mainloop,
                    y if joypad_keymap.contains_key(&y) => gameboy.keydown(joypad_keymap[&y]),
                    _ => {}
                },
                Event::KeyUp {
                    keycode: Some(x), ..
                } => match x {
                    y if joypad_keymap.contains_key(&y) => gameboy.keyup(joypad_keymap[&y]),
                    _ => {}
                },
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

    log::info!("Saving battery RAM if any");
    gameboy.save(save_file_path).unwrap();
}
