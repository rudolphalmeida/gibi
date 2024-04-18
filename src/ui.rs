use crate::GameFrame;
use eframe::egui::load::SizedTexture;
use eframe::egui::{menu, Color32, ColorImage, ImageSource, Key, RichText, TextureOptions};
use eframe::epaint::ImageDelta;
use eframe::glow::Context;
use eframe::{self, egui, CreationContext};
use gibi::cpu::Registers;
use gibi::framebuffer::access;
use gibi::gameboy::Gameboy;
use gibi::joypad::JoypadKeys;
use gibi::{
    framebuffer,
    ppu::{LCD_HEIGHT, LCD_WIDTH},
    EmulatorEvent,
};
use std::collections::HashMap;
use std::default::Default;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread::JoinHandle;

// Nearest neighbor filtering for the nice pixelated look
const TEXTURE_OPTIONS: TextureOptions = TextureOptions {
    magnification: egui::TextureFilter::Nearest,
    minification: egui::TextureFilter::Nearest,
    wrap_mode: egui::TextureWrapMode::ClampToEdge,
};

struct EmulatorCommCtx {
    emulation_thread: Option<JoinHandle<()>>,
    command_tx: mpsc::SyncSender<EmulatorCommand>,
    event_rc: mpsc::Receiver<EmulatorEvent>,
    tex: egui::TextureHandle,
    frame_reader: access::AccessR<GameFrame>,
}

struct UiCommCtx {
    frame_writer: access::AccessW<GameFrame>,
    command_rc: mpsc::Receiver<EmulatorCommand>,
    event_tx: mpsc::Sender<EmulatorEvent>,
}

fn spawn(rom_path: &PathBuf, ctx: &egui::Context) -> io::Result<EmulatorCommCtx> {
    let tex = ctx.load_texture(
        "game-image",
        ColorImage::new([LCD_WIDTH, LCD_HEIGHT], Color32::BLACK),
        TEXTURE_OPTIONS,
    );

    let rom = std::fs::read(rom_path)?;
    let save_file_path = rom_path.with_extension(".sav");
    let ram = std::fs::read(&save_file_path).ok();

    let (frame_reader, frame_writer) = framebuffer::buffers::triple::new::<GameFrame>();
    let (command_tx, command_rc) = mpsc::sync_channel(0);
    let (event_tx, event_rc) = mpsc::channel();
    let emulation_thread = {
        std::thread::Builder::new()
            .name("emulation-thread".to_owned())
            .spawn(move || {
                log::info!("Spawned new emulation thread");
                EmulationThread::new(
                    UiCommCtx {
                        frame_writer,
                        command_rc,
                        event_tx,
                    },
                    rom,
                    ram,
                    save_file_path,
                )
                .run();
                log::info!("Terminating emulation thread");
            })
            .expect("Failed to spawn emulation thread")
    };

    Ok(EmulatorCommCtx {
        emulation_thread: Some(emulation_thread),
        command_tx,
        event_rc,
        tex,
        frame_reader,
    })
}

#[derive(Default, Debug, Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize)]
enum Panel {
    #[default]
    Cpu,
    Ppu,
    Memory,
    Nametables,
    Cartridge,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct GameboyApp {
    game_scale_factor: f32,
    recent_roms: Vec<PathBuf>,
    open_panel: Panel,

    #[serde(skip)]
    cpu_registers: Option<Registers>,
    #[serde(skip)]
    comm_ctx: Option<EmulatorCommCtx>,
}

impl GameboyApp {
    pub fn new(cc: &CreationContext) -> Self {
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Self {
            game_scale_factor: 5.0,
            ..Self::default()
        }
    }

    fn send_message(&self, msg: EmulatorCommand) {
        if let Some(comm_ctx) = self.comm_ctx.as_ref() {
            comm_ctx
                .command_tx
                .send(msg)
                .expect("Failed to send message to thread");
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
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
                self.send_message(EmulatorCommand::KeyPressed(joypad_key));
            }

            if ctx.input(|i| i.key_released(key)) {
                self.send_message(EmulatorCommand::KeyReleased(joypad_key));
            }
        }
    }

    fn show_debug_ui(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("main-menu").show(ctx, |ui| {
            self.show_main_menu(ui, ctx, frame);
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
                                self.show_cpu_registers(ui, cpu_registers);
                            } else {
                                self.show_cpu_registers(ui, Registers::default());
                            }
                        }
                        Panel::Ppu => {}
                        Panel::Memory => {}
                        Panel::Nametables => {}
                        Panel::Cartridge => {}
                    }

                    ui.separator();
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                if let Some(comm_ctx) = self.comm_ctx.as_ref() {
                    let tex = &comm_ctx.tex;
                    ui.add(egui::Image::new(ImageSource::Texture(SizedTexture::new(
                        tex,
                        tex.size_vec2() * self.game_scale_factor,
                    ))));
                }
            })
        });
    }

    fn show_main_menu(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
    ) {
        menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open").clicked() {
                    // TODO: Stop running emulation if clicking
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        match spawn(&path, ctx) {
                            Ok(comm_ctx) => self.comm_ctx = Some(comm_ctx),
                            Err(err) => log::error!("Failed to load ROM file: {:?}", err),
                        }
                        self.recent_roms.push(path);
                    }
                }
                ui.menu_button("Open Recent" , |ui| {
                    for path in &self.recent_roms {
                        if (ui.button(path.file_name().unwrap().to_str().unwrap())).clicked() {
                            // TODO: Stop running emulation if clicking
                            match spawn(path, ctx) {
                                Ok(comm_ctx) => self.comm_ctx = Some(comm_ctx),
                                Err(err) => log::error!("Failed to load ROM fxxxxile: {:?}", err),
                            }
                        }
                    }
                });
                if ui.button("Exit").clicked() {
                    self.send_message(EmulatorCommand::Exit);
                }
            });

            ui.menu_button("Emulation", |ui| {
                if ui.button("Start").clicked() {
                    self.send_message(EmulatorCommand::Start);
                }
                if ui.button("Pause").clicked() {
                    self.send_message(EmulatorCommand::Pause);
                }
                if ui.button("Stop").clicked() {
                    self.send_message(EmulatorCommand::Stop);
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
                    if ui.button("6x").clicked() {
                        self.game_scale_factor = 6.0;
                    }
                    if ui.button("7x").clicked() {
                        self.game_scale_factor = 7.0;
                    }
                    if ui.button("8x").clicked() {
                        self.game_scale_factor = 8.0;
                    }
                });
                egui::widgets::global_dark_light_mode_buttons(ui);
            });
        });
    }

    fn show_cpu_registers(&mut self, ui: &mut egui::Ui, cpu_registers: Registers) {
        egui::Grid::new("cpu_registers_grid")
            .num_columns(4)
            .spacing([0.0, 20.0])
            .min_col_width(100.0)
            .striped(true)
            .show(ui, |ui| {
                let af = cpu_registers.get_af();
                let [a, f] = af.to_be_bytes();
                ui.label(RichText::new("A").strong());
                ui.label(format!("{:#04X}", a));
                ui.label(RichText::new("F").strong());
                ui.label(format!("{:#04X}", f));
                ui.end_row();

                let bc = cpu_registers.get_bc();
                let [b, c] = bc.to_be_bytes();
                ui.label(RichText::new("B").strong());
                ui.label(format!("{:#04X}", b));
                ui.label(RichText::new("C").strong());
                ui.label(format!("{:#04X}", c));
                ui.end_row();

                let de = cpu_registers.get_de();
                let [d, e] = de.to_be_bytes();
                ui.label(RichText::new("D").strong());
                ui.label(format!("{:#04X}", d));
                ui.label(RichText::new("E").strong());
                ui.label(format!("{:#04X}", e));
                ui.end_row();

                let hl = cpu_registers.get_hl();
                let [h, l] = hl.to_be_bytes();
                ui.label(RichText::new("H").strong());
                ui.label(format!("{:#04X}", h));
                ui.label(RichText::new("L").strong());
                ui.label(format!("{:#04X}", l));
                ui.end_row();

                ui.label(RichText::new("SP").strong());
                ui.label(format!("{:#06X}", cpu_registers.sp));
                ui.label(RichText::new("PC").strong());
                ui.label(format!("{:#06X}", cpu_registers.pc));
                ui.end_row();
            });

        ui.separator();

        ui.columns(5, |columns| {
            let f = cpu_registers.f;
            columns[0].label("Flags:");
            let zero_label = if f.zero {
                RichText::new("Z").strong()
            } else {
                RichText::new("Z").weak()
            };
            columns[1].label(zero_label);

            let sub_label = if f.negative {
                RichText::new("N").strong()
            } else {
                RichText::new("N").weak()
            };
            columns[2].label(sub_label);

            let half_carry_label = if f.half_carry {
                RichText::new("H").strong()
            } else {
                RichText::new("H").weak()
            };
            columns[3].label(half_carry_label);

            let carry_label = if f.carry {
                RichText::new("C").strong()
            } else {
                RichText::new("C").weak()
            };
            columns[4].label(carry_label);
        });
    }
}

impl eframe::App for GameboyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.handle_input(ctx);

        self.send_message(EmulatorCommand::RunFrame);
        self.send_message(EmulatorCommand::SendDebugData);

        if let Some(comm_ctx) = self.comm_ctx.as_mut() {
            while let Ok(event) = comm_ctx.event_rc.try_recv() {
                match event {
                    EmulatorEvent::CompletedFrame => {
                        let frame = comm_ctx.frame_reader.get().read().data.as_slice();
                        let frame_slice = unsafe { to_byte_slice(frame) };
                        let image = ColorImage::from_rgba_unmultiplied(
                            [LCD_WIDTH, LCD_HEIGHT],
                            frame_slice,
                        );
                        let delta = ImageDelta::full(image, TEXTURE_OPTIONS);
                        ctx.tex_manager().write().set(comm_ctx.tex.id(), delta);
                    }
                    EmulatorEvent::CpuRegisters(cpu_registers) => {
                        self.cpu_registers = Some(cpu_registers)
                    }
                }
            }
        }

        self.show_debug_ui(ctx, frame);
        ctx.request_repaint();
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn on_exit(&mut self, _gl: Option<&Context>) {
        self.send_message(EmulatorCommand::Exit);
        if let Some(comm_ctx) = self.comm_ctx.as_mut() {
            comm_ctx
                .emulation_thread
                .take()
                .map(JoinHandle::join)
                .unwrap()
                .unwrap();
        }
    }
}

#[inline(always)]
pub unsafe fn to_byte_slice<T>(data: &[T]) -> &[u8] {
    std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data))
}

#[derive(Debug)]
enum EmulatorCommand {
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

struct EmulationThread {
    gameboy: Gameboy,
    comm_ctx: UiCommCtx,
    paused: bool,
    save_file_path: PathBuf,
}

impl EmulationThread {
    fn new(
        comm_ctx: UiCommCtx,
        rom: Vec<u8>,
        ram: Option<Vec<u8>>,
        save_file_path: PathBuf,
    ) -> Self {
        let gameboy = Gameboy::new(rom, ram);
        Self {
            comm_ctx,
            gameboy,
            paused: true,
            save_file_path,
        }
    }

    fn run(&mut self) {
        loop {
            match self.comm_ctx.command_rc.recv() {
                Ok(m) => match m {
                    EmulatorCommand::Start if self.paused => self.paused = false,
                    EmulatorCommand::Start => {}
                    EmulatorCommand::RunFrame if !self.paused => {
                        self.gameboy.run_one_frame();
                        self.gameboy.write_frame(&mut self.comm_ctx.frame_writer);
                        self.comm_ctx
                            .event_tx
                            .send(EmulatorEvent::CompletedFrame)
                            .unwrap();
                    }
                    EmulatorCommand::RunFrame => {}
                    EmulatorCommand::Pause => self.paused = true,
                    EmulatorCommand::Stop => match self.gameboy.save(&self.save_file_path) {
                        Ok(msg) => log::info!("{msg}"),
                        Err(err) => log::error!("{err:?}"),
                    },
                    EmulatorCommand::KeyPressed(key) if !self.paused => self.gameboy.keydown(key),
                    EmulatorCommand::KeyPressed(_) => {}
                    EmulatorCommand::KeyReleased(key) if !self.paused => self.gameboy.keyup(key),
                    EmulatorCommand::KeyReleased(_) => {}
                    EmulatorCommand::Exit => {
                        // TODO: Save if a game is already running
                        log::info!("Received request to quit. Terminate emulation thread");
                        break;
                    }
                    EmulatorCommand::SendDebugData => {
                        let data = self.gameboy.send_debug_data();
                        self.comm_ctx
                            .event_tx
                            .send(EmulatorEvent::CpuRegisters(data))
                            .unwrap();
                    }
                },
                Err(e) => log::error!("{}", e),
            }
        }
    }
}
