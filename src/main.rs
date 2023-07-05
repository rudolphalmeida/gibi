use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    thread::JoinHandle,
};

use eframe::{
    egui::{self, menu, Key, TextureOptions},
    epaint::{Color32, ColorImage, ImageDelta},
    CreationContext,
};
use gibi::{
    cpu::Registers, gameboy::Gameboy, joypad::JoypadKeys, EmulatorEvent, Frame, GAMEBOY_HEIGHT,
    GAMEBOY_WIDTH,
};

const TEXTURE_OPTIONS: egui::TextureOptions = TextureOptions {
    magnification: egui::TextureFilter::Nearest,
    minification: egui::TextureFilter::Nearest,
};
const WIDTH: usize = GAMEBOY_WIDTH as usize;
const HEIGHT: usize = GAMEBOY_HEIGHT as usize;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1240.0, 760.0)),
        vsync: true,
        ..Default::default()
    };
    eframe::run_native(
        "GiBi: Gameboy Color Emulator",
        options,
        Box::new(|cc| Box::<GameboyApp>::new(GameboyApp::new(cc))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "ui_canvas", // hardcode it
                web_options,
                Box::new(|cc| Box::<GameboyApp>::new(GameboyApp::new(cc))),
            )
            .await
            .expect("failed to start eframe");
    });
}

#[derive(Debug)]
enum EmulatorCommand {
    LoadRom(PathBuf),
    SendDebugData,

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

struct EmulationContext {
    loaded_rom_file: Option<PathBuf>,
    gameboy: Option<Gameboy>,
    running: bool,

    frame: Frame,

    command_rc: mpsc::Receiver<EmulatorCommand>,
    event_tx: mpsc::Sender<EmulatorEvent>,
}

impl EmulationContext {
    fn new(
        frame: Frame,
        command_rc: mpsc::Receiver<EmulatorCommand>,
        event_tx: mpsc::Sender<EmulatorEvent>,
    ) -> Self {
        Self {
            loaded_rom_file: None,
            gameboy: None,
            running: false,
            frame,
            command_rc,
            event_tx,
        }
    }

    fn handle_message(&mut self, m: EmulatorCommand) -> bool {
        match m {
            EmulatorCommand::LoadRom(path) => {
                // TODO: Save if a game is already running
                let rom = std::fs::read(&path).unwrap();
                self.loaded_rom_file = Some(path);
                self.gameboy = Some(Gameboy::new(
                    Arc::clone(&self.frame),
                    rom,
                    None,
                    self.event_tx.clone(),
                ));
            }
            EmulatorCommand::Start if self.gameboy.is_some() => self.running = true,
            EmulatorCommand::Start => {}
            EmulatorCommand::RunFrame if self.running => {
                let gb_ctx = self.gameboy.as_mut().unwrap();
                gb_ctx.run_one_frame();
                self.event_tx.send(EmulatorEvent::CompletedFrame).unwrap();
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
            EmulatorCommand::SendDebugData if self.gameboy.is_some() => {
                self.gameboy.as_ref().unwrap().send_debug_data()
            }
            EmulatorCommand::SendDebugData => {}
            #[cfg(not(target_arch = "wasm32"))]
            EmulatorCommand::Exit => {
                // TODO: Save if a game is already running
                log::info!("Received request to quit. Terminate emulation thread");
                return true;
            }
            #[cfg(target_arch = "wasm32")]
            EmulatorCommand::Exit => {}
        }

        false
    }

    fn run(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        loop {
            match self.command_rc.recv() {
                Ok(m) => {
                    if self.handle_message(m) {
                        break;
                    }
                }
                Err(e) => log::error!("{}", e),
            }
        }

        #[cfg(target_arch = "wasm32")]
        while let Ok(m) = self.command_rc.try_recv() {
            if self.handle_message(m) {
                break;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Panel {
    Cpu,
    Ppu,
    Memory,
    Nametables,
    Cartridge,
}

struct GameboyApp {
    frame: Frame,
    tex: egui::TextureHandle,
    game_scale_factor: f32,

    // `Some(...)` when running native; None in `wasm`
    emulation_thread: Option<JoinHandle<()>>,
    // `Some(...)` when running `wasm`; None in native
    emulation_ctx: Option<EmulationContext>,

    command_tx: mpsc::Sender<EmulatorCommand>,
    event_rc: mpsc::Receiver<EmulatorEvent>,

    // Debugging Data
    open_panel: Panel,
    cpu_registers: Option<Registers>,
}

impl GameboyApp {
    fn new(cc: &CreationContext) -> Self {
        let tex = cc.egui_ctx.load_texture(
            "game-image",
            ColorImage::new([WIDTH, HEIGHT], Color32::BLACK),
            TEXTURE_OPTIONS,
        );

        let frame = Arc::new(Mutex::new(vec![0x00; WIDTH * HEIGHT * 4]));

        let (command_tx, command_rc) = mpsc::channel();
        let (event_tx, event_rc) = mpsc::channel();

        #[cfg(not(target_arch = "wasm32"))]
        let emulation_thread = Some({
            let frame = Arc::clone(&frame);
            std::thread::Builder::new()
                .name("emulation-thread".to_owned())
                .spawn(move || {
                    EmulationContext::new(Arc::clone(&frame), command_rc, event_tx).run();
                })
                .expect("Failed to spawn emulation thread")
        });
        #[cfg(target_arch = "wasm32")]
        let emulation_thread = None;

        #[cfg(not(target_arch = "wasm32"))]
        let emulation_ctx: Option<EmulationContext> = None;
        #[cfg(target_arch = "wasm32")]
        let emulation_ctx = Some(EmulationContext::new(
            Arc::clone(&frame),
            command_rc,
            event_tx,
        ));

        Self {
            tex,
            game_scale_factor: 5.0,
            emulation_thread,
            emulation_ctx,
            frame,
            command_tx,
            event_rc,
            cpu_registers: None,
            open_panel: Panel::Cpu,
        }
    }

    fn show_main_menu(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open").clicked() {
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.command_tx
                            .send(EmulatorCommand::LoadRom(path))
                            .unwrap();
                    }
                }
                if ui.button("Open Recent").clicked() {}

                #[cfg(not(target_arch = "wasm32"))]
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

            ui.menu_button("View", |ui| {
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
        });
    }
}

impl eframe::App for GameboyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("main-menu").show(ctx, |ui| {
            self.show_main_menu(ui, frame);
        });

        egui::SidePanel::left("debug-panel")
            .min_width(400.0)
            .resizable(false)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.open_panel, Panel::Cpu, "CPU");
                        ui.selectable_value(&mut self.open_panel, Panel::Ppu, "PPU");
                        ui.selectable_value(&mut self.open_panel, Panel::Cartridge, "Cartridge");
                        ui.selectable_value(&mut self.open_panel, Panel::Memory, "Memory");
                        ui.selectable_value(&mut self.open_panel, Panel::Nametables, "Nametables");
                    });
                    ui.separator();

                    match self.open_panel {
                        Panel::Cpu => {
                            if let Some(cpu_registers) = self.cpu_registers {
                                egui::Grid::new("cpu_registers_grid")
                                    .num_columns(2)
                                    .spacing([40.0, 4.0])
                                    .striped(true)
                                    .show(ui, |ui| {
                                        ui.label("AF");
                                        ui.label(format!("{:#06X}", cpu_registers.get_af()));
                                        ui.end_row();

                                        ui.label("BC");
                                        ui.label(format!("{:#06X}", cpu_registers.get_bc()));
                                        ui.end_row();

                                        ui.label("DE");
                                        ui.label(format!("{:#06X}", cpu_registers.get_de()));
                                        ui.end_row();

                                        ui.label("HL");
                                        ui.label(format!("{:#06X}", cpu_registers.get_hl()));
                                        ui.end_row();

                                        ui.label("SP");
                                        ui.label(format!("{:#06X}", cpu_registers.sp));
                                        ui.end_row();

                                        ui.label("PC");
                                        ui.label(format!("{:#06X}", cpu_registers.pc));
                                        ui.end_row();
                                    });
                            }
                        }
                        Panel::Ppu => {}
                        Panel::Memory => {}
                        Panel::Nametables => {}
                        Panel::Cartridge => {}
                    }
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
        self.command_tx
            .send(EmulatorCommand::SendDebugData)
            .unwrap();

        #[cfg(target_arch = "wasm32")]
        {
            self.emulation_ctx.as_mut().unwrap().run();
        }

        while let Ok(event) = self.event_rc.try_recv() {
            match event {
                EmulatorEvent::CompletedFrame => {
                    let locked_frame = self.frame.lock().unwrap();
                    let framebuffer = locked_frame.as_slice();
                    let image = ColorImage::from_rgba_unmultiplied([WIDTH, HEIGHT], framebuffer);
                    let delta = ImageDelta::full(image, TEXTURE_OPTIONS);
                    ctx.tex_manager().write().set(self.tex.id(), delta);
                }
                EmulatorEvent::CpuRegisters(cpu_registers) => {
                    self.cpu_registers = Some(cpu_registers)
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(egui::Image::new(
                &self.tex,
                self.tex.size_vec2() * self.game_scale_factor,
            ));
        });

        ctx.request_repaint();
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn on_close_event(&mut self) -> bool {
        self.command_tx.send(EmulatorCommand::Exit).unwrap();
        self.emulation_thread.take().map(JoinHandle::join);

        true
    }
}
