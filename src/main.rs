use std::{collections::HashMap, path::PathBuf, sync::mpsc, thread::JoinHandle};

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

#[derive(Debug)]
enum EmulatorCommand {
    LoadRom(PathBuf),

    // Emulation control
    Start,
    RunFrame,
    Pause,
    Stop,

    // Joypad events
    KeyPressed(JoypadKeys),
    KeyReleased(JoypadKeys),

    // Exit
    Exit,
}

enum EmulatorEvent {
    Frame(ImageDelta),
}

struct EmulationThread {
    loaded_rom_file: Option<PathBuf>,
    gameboy: Option<Gameboy>,
    running: bool,

    command_rc: mpsc::Receiver<EmulatorCommand>,
    event_tx: mpsc::Sender<EmulatorEvent>,
}

impl EmulationThread {
    fn new(
        command_rc: mpsc::Receiver<EmulatorCommand>,
        event_tx: mpsc::Sender<EmulatorEvent>,
    ) -> Self {
        Self {
            loaded_rom_file: None,
            gameboy: None,
            running: false,
            command_rc,
            event_tx,
        }
    }

    fn run(&mut self) {
        loop {
            match self.command_rc.recv() {
                Ok(m) => match m {
                    EmulatorCommand::LoadRom(path) => {
                        // TODO: Save if a game is already running
                        let rom = std::fs::read(&path).unwrap();
                        self.loaded_rom_file = Some(path);
                        self.gameboy = Some(Gameboy::new(rom, None));
                    }
                    EmulatorCommand::Start if self.gameboy.is_some() => self.running = true,
                    EmulatorCommand::Start => {}
                    EmulatorCommand::RunFrame if self.running => {
                        let gb_ctx = self.gameboy.as_mut().unwrap();
                        gb_ctx.run_one_frame();
                        let frame = gb_ctx.framebuffer();
                        let image = ColorImage::from_rgba_unmultiplied([WIDTH, HEIGHT], &frame);
                        let delta = ImageDelta::full(image, TEXTURE_OPTIONS);

                        self.event_tx.send(EmulatorEvent::Frame(delta)).unwrap();
                    }
                    EmulatorCommand::RunFrame => {}
                    EmulatorCommand::Pause => self.running = false,
                    EmulatorCommand::Stop => {
                        // TODO: Save if a game is already running
                        self.running = false;
                        self.gameboy = None;
                        self.loaded_rom_file = None;
                    }
                    EmulatorCommand::KeyPressed(key) if self.running => {
                        self.gameboy.as_mut().unwrap().keydown(key)
                    }
                    EmulatorCommand::KeyPressed(_) => {}
                    EmulatorCommand::KeyReleased(key) if self.running => {
                        self.gameboy.as_mut().unwrap().keyup(key)
                    }
                    EmulatorCommand::KeyReleased(_) => {}
                    EmulatorCommand::Exit => {
                        // TODO: Save if a game is already running
                        log::info!("Received request to quit. Terminate emulation thread");
                        break;
                    }
                },
                Err(e) => log::error!("{}", e),
            }
        }
    }
}

struct GameboyApp {
    tex: egui::TextureHandle,
    game_scale_factor: f32,

    emulation_thread: Option<JoinHandle<()>>,
    command_tx: mpsc::SyncSender<EmulatorCommand>,
    event_rc: mpsc::Receiver<EmulatorEvent>,
}

impl GameboyApp {
    fn new(cc: &CreationContext) -> Self {
        let tex = cc.egui_ctx.load_texture(
            "game-image",
            ColorImage::new([WIDTH, HEIGHT], Color32::BLACK),
            TEXTURE_OPTIONS,
        );

        let (command_tx, command_rc) = mpsc::sync_channel(0);
        let (event_tx, event_rc) = mpsc::channel();
        let emulation_thread = std::thread::Builder::new()
            .name("emulation-thread".to_owned())
            .spawn(move || {
                EmulationThread::new(command_rc, event_tx).run();
            })
            .expect("Failed to spawn emulation thread");

        Self {
            tex,
            game_scale_factor: 5.0,
            emulation_thread: Some(emulation_thread),
            command_tx,
            event_rc,
        }
    }

    fn show_main_menu(&mut self, ui: &mut egui::Ui) {
        menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.command_tx
                            .send(EmulatorCommand::LoadRom(path))
                            .unwrap();
                    }
                }
                if ui.button("Open Recent").clicked() {}
                if ui.button("Exit").clicked() {
                    self.command_tx.send(EmulatorCommand::Stop).unwrap();
                }
            });

            ui.menu_button("Emulation", |ui| {
                if ui.button("Start").clicked() {
                    self.command_tx.send(EmulatorCommand::Start).unwrap();
                }
                if ui.button("Pause").clicked() {
                    self.command_tx.send(EmulatorCommand::Pause).unwrap();
                }
                if ui.button("Stop").clicked() {
                    self.command_tx.send(EmulatorCommand::Stop).unwrap();
                }
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
                self.command_tx
                    .send(EmulatorCommand::KeyPressed(joypad_key))
                    .unwrap();
            }

            if ctx.input(|i| i.key_released(key)) {
                self.command_tx
                    .send(EmulatorCommand::KeyReleased(joypad_key))
                    .unwrap();
            }
        }

        self.command_tx.send(EmulatorCommand::RunFrame).unwrap();

        while let Ok(event) = self.event_rc.try_recv() {
            match event {
                EmulatorEvent::Frame(delta) => {
                    ctx.tex_manager().write().set(self.tex.id(), delta);

                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.add(egui::Image::new(
                            &self.tex,
                            self.tex.size_vec2() * self.game_scale_factor,
                        ));
                    });
                }
            }
        }

        ctx.request_repaint();
    }

    fn on_close_event(&mut self) -> bool {
        self.command_tx.send(EmulatorCommand::Exit).unwrap();
        self.emulation_thread.take().map(JoinHandle::join);

        true
    }
}
