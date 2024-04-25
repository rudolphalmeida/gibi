use crate::GameFrame;
use eframe::egui::load::SizedTexture;
use eframe::egui::{menu, Color32, ColorImage, ImageSource, Key, RichText, TextureOptions};
use eframe::epaint::ImageDelta;
use eframe::glow::Context;
use eframe::{self, egui, CreationContext};
use gibi::cartridge::CartridgeHeader;
use gibi::cpu::Registers;
use gibi::debug::{CpuDebug, ExecutedOpcode};
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

fn format_opcode(opcode: u8, arg1: u8, arg2: u8) -> String {
    match opcode {
        0 => format!("NOP "),
        1 => format!("LD BC, 0x{arg2:02X}{arg1:02X}"),
        2 => format!("LD (BC), A"),
        3 => format!("INC BC"),
        4 => format!("INC B"),
        5 => format!("DEC B"),
        6 => format!("LD B, 0x{arg1:02X}"),
        7 => format!("RLCA "),
        8 => format!("LD (0x{arg2:02X}{arg1:02X}), SP"),
        9 => format!("ADD HL, BC"),
        10 => format!("LD A, (BC)"),
        11 => format!("DEC BC"),
        12 => format!("INC C"),
        13 => format!("DEC C"),
        14 => format!("LD C, 0x{arg1:02X}"),
        15 => format!("RRCA "),
        16 => format!("STOP 0x{arg1:02X}"),
        17 => format!("LD DE, 0x{arg2:02X}{arg1:02X}"),
        18 => format!("LD (DE), A"),
        19 => format!("INC DE"),
        20 => format!("INC D"),
        21 => format!("DEC D"),
        22 => format!("LD D, 0x{arg1:02X}"),
        23 => format!("RLA "),
        24 => format!("JR 0x{arg1:02X}"),
        25 => format!("ADD HL, DE"),
        26 => format!("LD A, (DE)"),
        27 => format!("DEC DE"),
        28 => format!("INC E"),
        29 => format!("DEC E"),
        30 => format!("LD E, 0x{arg1:02X}"),
        31 => format!("RRA "),
        32 => format!("JR NZ, 0x{arg1:02X}"),
        33 => format!("LD HL, 0x{arg2:02X}{arg1:02X}"),
        34 => format!("LD (HL), A"),
        35 => format!("INC HL"),
        36 => format!("INC H"),
        37 => format!("DEC H"),
        38 => format!("LD H, 0x{arg1:02X}"),
        39 => format!("DAA "),
        40 => format!("JR Z, 0x{arg1:02X}"),
        41 => format!("ADD HL, HL"),
        42 => format!("LD A, (HL)"),
        43 => format!("DEC HL"),
        44 => format!("INC L"),
        45 => format!("DEC L"),
        46 => format!("LD L, 0x{arg1:02X}"),
        47 => format!("CPL "),
        48 => format!("JR NC, 0x{arg1:02X}"),
        49 => format!("LD SP, 0x{arg2:02X}{arg1:02X}"),
        50 => format!("LD (HL), A"),
        51 => format!("INC SP"),
        52 => format!("INC (HL)"),
        53 => format!("DEC (HL)"),
        54 => format!("LD (HL), 0x{arg1:02X}"),
        55 => format!("SCF "),
        56 => format!("JR C, 0x{arg1:02X}"),
        57 => format!("ADD HL, SP"),
        58 => format!("LD A, (HL)"),
        59 => format!("DEC SP"),
        60 => format!("INC A"),
        61 => format!("DEC A"),
        62 => format!("LD A, 0x{arg1:02X}"),
        63 => format!("CCF "),
        64 => format!("LD B, B"),
        65 => format!("LD B, C"),
        66 => format!("LD B, D"),
        67 => format!("LD B, E"),
        68 => format!("LD B, H"),
        69 => format!("LD B, L"),
        70 => format!("LD B, (HL)"),
        71 => format!("LD B, A"),
        72 => format!("LD C, B"),
        73 => format!("LD C, C"),
        74 => format!("LD C, D"),
        75 => format!("LD C, E"),
        76 => format!("LD C, H"),
        77 => format!("LD C, L"),
        78 => format!("LD C, (HL)"),
        79 => format!("LD C, A"),
        80 => format!("LD D, B"),
        81 => format!("LD D, C"),
        82 => format!("LD D, D"),
        83 => format!("LD D, E"),
        84 => format!("LD D, H"),
        85 => format!("LD D, L"),
        86 => format!("LD D, (HL)"),
        87 => format!("LD D, A"),
        88 => format!("LD E, B"),
        89 => format!("LD E, C"),
        90 => format!("LD E, D"),
        91 => format!("LD E, E"),
        92 => format!("LD E, H"),
        93 => format!("LD E, L"),
        94 => format!("LD E, (HL)"),
        95 => format!("LD E, A"),
        96 => format!("LD H, B"),
        97 => format!("LD H, C"),
        98 => format!("LD H, D"),
        99 => format!("LD H, E"),
        100 => format!("LD H, H"),
        101 => format!("LD H, L"),
        102 => format!("LD H, (HL)"),
        103 => format!("LD H, A"),
        104 => format!("LD L, B"),
        105 => format!("LD L, C"),
        106 => format!("LD L, D"),
        107 => format!("LD L, E"),
        108 => format!("LD L, H"),
        109 => format!("LD L, L"),
        110 => format!("LD L, (HL)"),
        111 => format!("LD L, A"),
        112 => format!("LD (HL), B"),
        113 => format!("LD (HL), C"),
        114 => format!("LD (HL), D"),
        115 => format!("LD (HL), E"),
        116 => format!("LD (HL), H"),
        117 => format!("LD (HL), L"),
        118 => format!("HALT "),
        119 => format!("LD (HL), A"),
        120 => format!("LD A, B"),
        121 => format!("LD A, C"),
        122 => format!("LD A, D"),
        123 => format!("LD A, E"),
        124 => format!("LD A, H"),
        125 => format!("LD A, L"),
        126 => format!("LD A, (HL)"),
        127 => format!("LD A, A"),
        128 => format!("ADD A, B"),
        129 => format!("ADD A, C"),
        130 => format!("ADD A, D"),
        131 => format!("ADD A, E"),
        132 => format!("ADD A, H"),
        133 => format!("ADD A, L"),
        134 => format!("ADD A, (HL)"),
        135 => format!("ADD A, A"),
        136 => format!("ADC A, B"),
        137 => format!("ADC A, C"),
        138 => format!("ADC A, D"),
        139 => format!("ADC A, E"),
        140 => format!("ADC A, H"),
        141 => format!("ADC A, L"),
        142 => format!("ADC A, (HL)"),
        143 => format!("ADC A, A"),
        144 => format!("SUB B"),
        145 => format!("SUB C"),
        146 => format!("SUB D"),
        147 => format!("SUB E"),
        148 => format!("SUB H"),
        149 => format!("SUB L"),
        150 => format!("SUB (HL)"),
        151 => format!("SUB A"),
        152 => format!("SBC A, B"),
        153 => format!("SBC A, C"),
        154 => format!("SBC A, D"),
        155 => format!("SBC A, E"),
        156 => format!("SBC A, H"),
        157 => format!("SBC A, L"),
        158 => format!("SBC A, (HL)"),
        159 => format!("SBC A, A"),
        160 => format!("AND B"),
        161 => format!("AND C"),
        162 => format!("AND D"),
        163 => format!("AND E"),
        164 => format!("AND H"),
        165 => format!("AND L"),
        166 => format!("AND (HL)"),
        167 => format!("AND A"),
        168 => format!("XOR B"),
        169 => format!("XOR C"),
        170 => format!("XOR D"),
        171 => format!("XOR E"),
        172 => format!("XOR H"),
        173 => format!("XOR L"),
        174 => format!("XOR (HL)"),
        175 => format!("XOR A"),
        176 => format!("OR B"),
        177 => format!("OR C"),
        178 => format!("OR D"),
        179 => format!("OR E"),
        180 => format!("OR H"),
        181 => format!("OR L"),
        182 => format!("OR (HL)"),
        183 => format!("OR A"),
        184 => format!("CP B"),
        185 => format!("CP C"),
        186 => format!("CP D"),
        187 => format!("CP E"),
        188 => format!("CP H"),
        189 => format!("CP L"),
        190 => format!("CP (HL)"),
        191 => format!("CP A"),
        192 => format!("RET NZ"),
        193 => format!("POP BC"),
        194 => format!("JP NZ, (0x{arg2:02X}{arg1:02X})"),
        195 => format!("JP (0x{arg2:02X}{arg1:02X})"),
        196 => format!("CALL NZ, (0x{arg2:02X}{arg1:02X})"),
        197 => format!("PUSH BC"),
        198 => format!("ADD A, 0x{arg1:02X}"),
        199 => format!("RST 0x00"),
        200 => format!("RET Z"),
        201 => format!("RET "),
        202 => format!("JP Z, (0x{arg2:02X}{arg1:02X})"),
        203 => format!("PREFIX "),
        204 => format!("CALL Z, (0x{arg2:02X}{arg1:02X})"),
        205 => format!("CALL (0x{arg2:02X}{arg1:02X})"),
        206 => format!("ADC A, 0x{arg1:02X}"),
        207 => format!("RST 0x08"),
        208 => format!("RET NC"),
        209 => format!("POP DE"),
        210 => format!("JP NC, (0x{arg2:02X}{arg1:02X})"),
        211 => format!("ILLEGAL_D3 "),
        212 => format!("CALL NC, (0x{arg2:02X}{arg1:02X})"),
        213 => format!("PUSH DE"),
        214 => format!("SUB 0x{arg1:02X}"),
        215 => format!("RST 0x10"),
        216 => format!("RET C"),
        217 => format!("RETI "),
        218 => format!("JP C, (0x{arg2:02X}{arg1:02X})"),
        219 => format!("ILLEGAL_DB "),
        220 => format!("CALL C, (0x{arg2:02X}{arg1:02X})"),
        221 => format!("ILLEGAL_DD "),
        222 => format!("SBC A, 0x{arg1:02X}"),
        223 => format!("RST 0x18"),
        224 => format!("LDH (0xFF{arg1:02X}), A"),
        225 => format!("POP HL"),
        226 => format!("LD C, A"),
        227 => format!("ILLEGAL_E3 "),
        228 => format!("ILLEGAL_E4 "),
        229 => format!("PUSH HL"),
        230 => format!("AND 0x{arg1:02X}"),
        231 => format!("RST 0x20"),
        232 => format!("ADD SP, 0x{arg1:02X}"),
        233 => format!("JP HL"),
        234 => format!("LD (0x{arg2:02X}{arg1:02X}), A"),
        235 => format!("ILLEGAL_EB "),
        236 => format!("ILLEGAL_EC "),
        237 => format!("ILLEGAL_ED "),
        238 => format!("XOR 0x{arg1:02X}"),
        239 => format!("RST 0x28"),
        240 => format!("LDH A, (0xFF{arg1:02X})"),
        241 => format!("POP AF"),
        242 => format!("LD A, C"),
        243 => format!("DI "),
        244 => format!("ILLEGAL_F4 "),
        245 => format!("PUSH AF"),
        246 => format!("OR 0x{arg1:02X}"),
        247 => format!("RST 0x30"),
        248 => format!("LD HL, SP, 0x{arg1:02X}"),
        249 => format!("LD SP, HL"),
        250 => format!("LD A, (0x{arg2:02X}{arg1:02X})"),
        251 => format!("EI "),
        252 => format!("ILLEGAL_FC "),
        253 => format!("ILLEGAL_FD "),
        254 => format!("CP 0x{arg1:02X}"),
        255 => format!("RST 0x38"),
    }
}

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
    cpu_debug: Option<CpuDebug>,
    #[serde(skip)]
    cart_header: Option<CartridgeHeader>,
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
                        Panel::Cpu => self.show_cpu_debug(ui),
                        Panel::Ppu => {}
                        Panel::Memory => {}
                        Panel::Nametables => {}
                        Panel::Cartridge => self.show_cart_info(ui),
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
                ui.menu_button("Open Recent", |ui| {
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

    fn show_cpu_debug(&mut self, ui: &mut egui::Ui) {
        let (cpu_registers, opcodes) = if let Some(ref cpu_debug) = self.cpu_debug {
            (cpu_debug.registers, cpu_debug.opcodes.as_slice())
        } else {
            (Registers::default(), ([] as [ExecutedOpcode; 0]).as_slice())
        };
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

        ui.separator();
        egui::Grid::new("cpu_opcodes_grid")
            .num_columns(2)
            .spacing([40.0, 20.0])
            .min_col_width(200.0)
            .striped(true)
            .show(ui, |ui| {
                for executed_opcode in opcodes {
                    let ExecutedOpcode {
                        pc,
                        opcode,
                        arg1,
                        arg2,
                    } = *executed_opcode;
                    ui.label(format!("{:#06X}", pc));
                    ui.label(format_opcode(opcode, arg1, arg2));
                    ui.end_row();
                }
            });
    }

    fn show_cart_info(&self, ui: &mut egui::Ui) {
        if self.cart_header.is_none() {
            return;
        }

        let cart_header = self.cart_header.as_ref().unwrap();
        egui::Grid::new("cart_info_grid")
            .num_columns(2)
            .spacing([40.0, 20.0])
            .min_col_width(200.0)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Title");
                ui.label(&cart_header.title);
                ui.end_row();

                ui.label("Manufacturer Code");
                ui.label(&cart_header.manufacturer_code);
                ui.end_row();

                ui.label("Supported Hardware");
                ui.label(match cart_header.hardware_supported {
                    gibi::HardwareSupport::CgbOnly => "CGB Only",
                    gibi::HardwareSupport::DmgCgb => "DMG & CGB",
                    gibi::HardwareSupport::DmgCompat => "DMG Compat",
                });
                ui.end_row();

                ui.label("MBC Configuration");
                ui.label(&cart_header.cart_type);
                ui.end_row();

                ui.label("ROM Size");
                ui.label(format!("{}", cart_header.rom_size()));
                ui.end_row();

                ui.label("ROM Banks");
                ui.label(format!("{}", cart_header.rom_banks()));
                ui.end_row();

                ui.label("RAM Size");
                ui.label(format!("{}", cart_header.ram_size()));
                ui.end_row();

                ui.label("RAM Banks");
                ui.label(format!("{}", cart_header.ram_banks()));
                ui.end_row();
            });
    }
}

impl eframe::App for GameboyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.handle_input(ctx);

        self.send_message(EmulatorCommand::RunFrame);
        self.send_message(EmulatorCommand::QueryDebug(self.open_panel));

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
                        self.cpu_debug = Some(cpu_registers)
                    }
                    EmulatorEvent::CartridgeInfo(cart_header) => {
                        self.cart_header = Some(cart_header)
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
    // Emulation control
    Start,
    RunFrame,
    Pause,
    Stop,

    // Debug
    QueryDebug(Panel),

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
        let (gameboy, cart_header) = Gameboy::new(rom, ram);
        comm_ctx
            .event_tx
            .send(EmulatorEvent::CartridgeInfo(cart_header))
            .unwrap();
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
                        let data = self.gameboy.load_cpu_debug();
                        self.comm_ctx
                            .event_tx
                            .send(EmulatorEvent::CpuRegisters(data))
                            .unwrap();
                    }
                    EmulatorCommand::QueryDebug(panel) => self.send_debug_for_panel(panel),
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
                },
                Err(e) => log::error!("{}", e),
            }
        }
    }

    fn send_debug_for_panel(&mut self, panel: Panel) {
        let debug = match panel {
            Panel::Cpu => EmulatorEvent::CpuRegisters(self.gameboy.load_cpu_debug()),
            _ => return,
        };
        self.comm_ctx.event_tx.send(debug).unwrap();
    }
}
