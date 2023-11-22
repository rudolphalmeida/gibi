use crate::{EmulationThread, EmulatorCommand, GameFrame};
use eframe::egui::load::SizedTexture;
use eframe::egui::{menu, Color32, ColorImage, ImageSource, Key, RichText, TextureOptions};
use eframe::epaint::ImageDelta;
use eframe::{egui, CreationContext};
use gibi::cpu::Registers;
use gibi::framebuffer::access;
use gibi::joypad::JoypadKeys;
use gibi::{
    framebuffer,
    ppu::{LCD_HEIGHT, LCD_WIDTH},
    EmulatorEvent,
};
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread::JoinHandle;

// Nearest neighbor filtering for the nice pixelated look
const TEXTURE_OPTIONS: TextureOptions = TextureOptions {
    magnification: egui::TextureFilter::Nearest,
    minification: egui::TextureFilter::Nearest,
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Panel {
    Cpu,
    Ppu,
    Memory,
    Nametables,
    Cartridge,
}

pub struct GameboyApp {
    tex: egui::TextureHandle,
    game_scale_factor: f32,

    frame_reader: access::AccessR<GameFrame>,

    emulation_thread: Option<JoinHandle<()>>,
    command_tx: mpsc::SyncSender<EmulatorCommand>,
    event_rc: mpsc::Receiver<EmulatorEvent>,

    // Debugging Data
    open_panel: Panel,
    cpu_registers: Option<Registers>,
}

impl GameboyApp {
    pub fn new(cc: &CreationContext) -> Self {
        let tex = cc.egui_ctx.load_texture(
            "game-image",
            ColorImage::new([LCD_WIDTH, LCD_HEIGHT], Color32::BLACK),
            TEXTURE_OPTIONS,
        );

        let (frame_reader, frame_writer) = framebuffer::buffers::triple::new::<GameFrame>();

        let (command_tx, command_rc) = mpsc::sync_channel(0);
        let (event_tx, event_rc) = mpsc::channel();
        let emulation_thread = {
            std::thread::Builder::new()
                .name("emulation-thread".to_owned())
                .spawn(move || {
                    log::info!("Spawned emulation thread");
                    EmulationThread::new(frame_writer, command_rc, event_tx).run();
                    log::info!("Exiting emulation thread");
                })
                .expect("Failed to spawn emulation thread")
        };

        Self {
            frame_reader,
            tex,
            game_scale_factor: 5.0,
            emulation_thread: Some(emulation_thread),
            command_tx,
            event_rc,
            cpu_registers: None,
            open_panel: Panel::Cpu,
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
    }

    fn show_debug_ui(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
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
                                self.show_cpu_registers(ui, cpu_registers);
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
                ui.add(egui::Image::new(ImageSource::Texture(SizedTexture::new(
                    &self.tex,
                    self.tex.size_vec2() * self.game_scale_factor,
                ))));
            })
        });
    }

    fn show_main_menu(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        let rom_name = path.file_name().unwrap();
                        frame.set_window_title(rom_name.to_str().unwrap());
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

        self.command_tx.send(EmulatorCommand::RunFrame).unwrap();
        self.command_tx
            .send(EmulatorCommand::SendDebugData)
            .unwrap();

        while let Ok(event) = self.event_rc.try_recv() {
            match event {
                EmulatorEvent::CompletedFrame => {
                    // let locked_frame = self.frame.lock().unwrap();
                    // let framebuffer = locked_frame.as_slice();

                    let frame = self.frame_reader.get().read().data.as_slice();
                    let frame_slice = unsafe { to_byte_slice(frame) };
                    let image =
                        ColorImage::from_rgba_unmultiplied([LCD_WIDTH, LCD_HEIGHT], frame_slice);
                    let delta = ImageDelta::full(image, TEXTURE_OPTIONS);
                    ctx.tex_manager().write().set(self.tex.id(), delta);
                }
                EmulatorEvent::CpuRegisters(cpu_registers) => {
                    self.cpu_registers = Some(cpu_registers)
                }
            }
        }

        self.show_debug_ui(ctx, frame);
        ctx.request_repaint();
    }

    fn on_close_event(&mut self) -> bool {
        self.command_tx.send(EmulatorCommand::Exit).unwrap();
        self.emulation_thread.take().map(JoinHandle::join);

        true
    }
}

#[inline(always)]
pub unsafe fn to_byte_slice<T>(data: &[T]) -> &[u8] {
    std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data))
}