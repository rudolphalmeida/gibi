use std::{collections::HashMap, path::PathBuf};

use eframe::{
    egui::{self, menu, Key, TextureOptions},
    epaint::{Color32, ColorImage, ImageDelta},
    CreationContext,
};
use gibi::{gameboy::Gameboy, joypad::JoypadKeys, GAMEBOY_HEIGHT, GAMEBOY_WIDTH};

const TEXTURE_OPTIONS: egui::TextureOptions = TextureOptions {
    magnification: egui::TextureFilter::Nearest,
    minification: egui::TextureFilter::Nearest,
};
const WIDTH: usize = GAMEBOY_WIDTH as usize;
const HEIGHT: usize = GAMEBOY_HEIGHT as usize;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1250.0, 760.0)),
        vsync: true,
        ..Default::default()
    };
    eframe::run_native(
        "GiBi: Gameboy Color Emulator",
        options,
        Box::new(|cc| Box::<GameboyApp>::new(GameboyApp::new(cc))),
    )
}

struct GameboyApp {
    loaded_rom_file: Option<PathBuf>,
    gameboy: Option<Gameboy>,
    tex: egui::TextureHandle,
    game_scale_factor: f32,
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

            ui.menu_button("Scale", |ui| {
                if ui.button("1x").clicked() {
                    self.game_scale_factor = 1.0;
                }
                if ui.button("2x").clicked() {
                    self.game_scale_factor = 2.0;
                }
                if ui.button("3x").clicked() {
                    self.game_scale_factor = 3.0;
                }
                if ui.button("4x").clicked() {
                    self.game_scale_factor = 4.0;
                }
                if ui.button("5x").clicked() {
                    self.game_scale_factor = 5.0;
                }
            });
        });
    }
}

impl GameboyApp {
    fn new(cc: &CreationContext) -> Self {
        let tex = cc.egui_ctx.load_texture(
            "game-image",
            ColorImage::new([WIDTH, HEIGHT], Color32::BLACK),
            TEXTURE_OPTIONS,
        );
        Self {
            loaded_rom_file: None,
            gameboy: None,
            tex,
            game_scale_factor: 5.0,
        }
    }
}

impl eframe::App for GameboyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("debug-panel").show(ctx, |ui| {
            self.show_main_menu(ui);
        });

        egui::SidePanel::left("left-debug-panel")
            .min_width(200.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::CollapsingHeader::new("CPU")
                        .default_open(true)
                        .show(ui, |_ui| {});
                    egui::CollapsingHeader::new("Memory")
                        .default_open(true)
                        .show(ui, |_ui| {});
                });
            });

        egui::SidePanel::right("right-debug-panel")
            .min_width(200.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::CollapsingHeader::new("PPU")
                        .default_open(true)
                        .show(ui, |_ui| {});
                    egui::CollapsingHeader::new("Nametables")
                        .default_open(true)
                        .show(ui, |_ui| {});
                });
            });

        match &mut self.gameboy {
            Some(gb_ctx) => {
                let joypad_keymap: HashMap<Key, JoypadKeys> = HashMap::from([
                    (Key::Z, JoypadKeys::B),
                    (Key::X, JoypadKeys::A),
                    (Key::N, JoypadKeys::Select),
                    (Key::M, JoypadKeys::Start),
                    (Key::ArrowDown, JoypadKeys::Down),
                    (Key::ArrowUp, JoypadKeys::Up),
                    (Key::ArrowLeft, JoypadKeys::Left),
                    (Key::ArrowRight, JoypadKeys::Right),
                ]);

                for (key, joypad_key) in joypad_keymap {
                    if ctx.input(|i| i.key_down(key)) {
                        gb_ctx.keydown(joypad_key);
                    }

                    if ctx.input(|i| i.key_released(key)) {
                        gb_ctx.keyup(joypad_key);
                    }
                }

                gb_ctx.run_one_frame();
                let frame = gb_ctx.framebuffer();
                let image = ColorImage::from_rgba_unmultiplied([WIDTH, HEIGHT], &frame);
                let delta = ImageDelta::full(image, TEXTURE_OPTIONS);
                ctx.tex_manager().write().set(self.tex.id(), delta);

                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add(egui::Image::new(
                        &self.tex,
                        self.tex.size_vec2() * self.game_scale_factor,
                    ));
                });

                ctx.request_repaint();
            }
            None => {
                egui::CentralPanel::default().show(ctx, |_ui| {});
            }
        }
    }
}
