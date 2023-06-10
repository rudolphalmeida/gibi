use eframe::egui::{self, menu};
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

impl GameboyApp {
    fn show_main_menu(&mut self, ui: &mut egui::Ui) {
        menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open").clicked() {}
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
        Self {}
    }
}

impl eframe::App for GameboyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| self.show_main_menu(ui));
    }
}
