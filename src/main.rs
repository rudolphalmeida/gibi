use eframe::egui;
use gibi::{GAMEBOY_HEIGHT, GAMEBOY_WIDTH};

mod options;
mod ui;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(GAMEBOY_WIDTH, GAMEBOY_HEIGHT)),
        ..Default::default()
    };
    eframe::run_native(
        "GiBi: Gameboy Color Emulator",
        options,
        Box::new(|_cc| Box::<GameboyApp>::default()),
    )
}

struct GameboyApp {}

impl Default for GameboyApp {
    fn default() -> Self {
        Self {}
    }
}

impl eframe::App for GameboyApp {
    fn update(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {}
}
