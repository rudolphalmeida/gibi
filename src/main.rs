use std::path::PathBuf;

use eframe::egui::{self, menu};
use gibi::{gameboy::Gameboy, GAMEBOY_HEIGHT, GAMEBOY_WIDTH};

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

struct GameboyApp {
    loaded_rom_file: Option<PathBuf>,
    gameboy: Option<Gameboy>,
    pixels: Vec<u8>,
}

impl GameboyApp {
    fn show_main_menu(&mut self, ui: &mut egui::Ui) {
        menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        let rom = std::fs::read(&path).unwrap();

                        self.loaded_rom_file = Some(path);
                        self.gameboy = Some(Gameboy::new(rom, None));
                    }
                }
                if ui.button("Open Recent").clicked() {}
                if ui.button("Exit").clicked() {}
            });

            ui.menu_button("Emulation", |ui| {
                if ui.button("Start").clicked() {}
                if ui.button("Pause").clicked() {}
                if ui.button("Stop").clicked() {}
            });

            ui.menu_button("View", |ui| {
                ui.menu_button("Scale", |ui| {
                    if ui.button("1x").clicked() {}
                    if ui.button("2x").clicked() {}
                    if ui.button("3x").clicked() {}
                });

                if ui.button("CPU").clicked() {}
                if ui.button("PPU").clicked() {}
                if ui.button("Background Maps").clicked() {}
            });
        });
    }
}

impl Default for GameboyApp {
    fn default() -> Self {
        Self {
            loaded_rom_file: None,
            gameboy: None,
            // RGBA pixels
            pixels: vec![0x00; GAMEBOY_WIDTH as usize * GAMEBOY_HEIGHT as usize * 4],
        }
    }
}

impl eframe::App for GameboyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        match &mut self.gameboy {
            Some(gb_ctx) => {
                gb_ctx.run_one_frame();
                gb_ctx.copy_framebuffer_to_draw_target(&mut self.pixels);
                ctx.request_repaint();
            }
            None => {}
        }

        egui::CentralPanel::default().show(ctx, |ui| self.show_main_menu(ui));
    }
}
