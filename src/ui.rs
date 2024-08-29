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
        0 => "NOP ".to_string(),
        1 => format!("LD BC, 0x{arg2:02X}{arg1:02X}"),
        2 => "LD (BC), A".to_string(),
        3 => "INC BC".to_string(),
        4 => "INC B".to_string(),
        5 => "DEC B".to_string(),
        6 => format!("LD B, 0x{arg1:02X}"),
        7 => "RCA ".to_string(),
        8 => format!("LD (0x{arg2:02X}{arg1:02X}), SP"),
        9 => "ADD HL, BC".to_string(),
        10 => "LD A, (BC)".to_string(),
        11 => "DEC BC".to_string(),
        12 => "INC C".to_string(),
        13 => "DEC C".to_string(),
        14 => format!("LD C, 0x{arg1:02X}"),
        15 => "RCA ".to_string(),
        16 => format!("STOP 0x{arg1:02X}"),
        17 => format!("LD DE, 0x{arg2:02X}{arg1:02X}"),
        18 => "LD (DE), A".to_string(),
        19 => "INC DE".to_string(),
        20 => "INC D".to_string(),
        21 => "DEC D".to_string(),
        22 => format!("LD D, 0x{arg1:02X}"),
        23 => "RLA ".to_string(),
        24 => format!("JR 0x{arg1:02X}"),
        25 => "ADD HL, DE".to_string(),
        26 => "LD A, (DE)".to_string(),
        27 => "DEC DE".to_string(),
        28 => "INC E".to_string(),
        29 => "DEC E".to_string(),
        30 => format!("LD E, 0x{arg1:02X}"),
        31 => "RRA ".to_string(),
        32 => format!("JR NZ, 0x{arg1:02X}"),
        33 => format!("LD HL, 0x{arg2:02X}{arg1:02X}"),
        34 => "LD (HL), A".to_string(),
        35 => "INC HL".to_string(),
        36 => "INC H".to_string(),
        37 => "DEC H".to_string(),
        38 => format!("LD H, 0x{arg1:02X}"),
        39 => "DAA ".to_string(),
        40 => format!("JR Z, 0x{arg1:02X}"),
        41 => "ADD HL, HL".to_string(),
        42 => "LD A, (HL)".to_string(),
        43 => "DEC HL".to_string(),
        44 => "INC L".to_string(),
        45 => "DEC L".to_string(),
        46 => format!("LD L, 0x{arg1:02X}"),
        47 => "CPL ".to_string(),
        48 => format!("JR NC, 0x{arg1:02X}"),
        49 => format!("LD SP, 0x{arg2:02X}{arg1:02X}"),
        50 => "LD (HL), A".to_string(),
        51 => "INC SP".to_string(),
        52 => "INC (HL)".to_string(),
        53 => "DEC (HL)".to_string(),
        54 => format!("LD (HL), 0x{arg1:02X}"),
        55 => "SCF ".to_string(),
        56 => format!("JR C, 0x{arg1:02X}"),
        57 => "ADD HL, SP".to_string(),
        58 => "LD A, (HL)".to_string(),
        59 => "DEC SP".to_string(),
        60 => "INC A".to_string(),
        61 => "DEC A".to_string(),
        62 => format!("LD A, 0x{arg1:02X}"),
        63 => "CCF ".to_string(),
        64 => "LD B, B".to_string(),
        65 => "LD B, C".to_string(),
        66 => "LD B, D".to_string(),
        67 => "LD B, E".to_string(),
        68 => "LD B, H".to_string(),
        69 => "LD B, L".to_string(),
        70 => "LD B, (HL)".to_string(),
        71 => "LD B, A".to_string(),
        72 => "LD C, B".to_string(),
        73 => "LD C, C".to_string(),
        74 => "LD C, D".to_string(),
        75 => "LD C, E".to_string(),
        76 => "LD C, H".to_string(),
        77 => "LD C, L".to_string(),
        78 => "LD C, (HL)".to_string(),
        79 => "LD C, A".to_string(),
        80 => "LD D, B".to_string(),
        81 => "LD D, C".to_string(),
        82 => "LD D, D".to_string(),
        83 => "LD D, E".to_string(),
        84 => "LD D, H".to_string(),
        85 => "LD D, L".to_string(),
        86 => "LD D, (HL)".to_string(),
        87 => "LD D, A".to_string(),
        88 => "LD E, B".to_string(),
        89 => "LD E, C".to_string(),
        90 => "LD E, D".to_string(),
        91 => "LD E, E".to_string(),
        92 => "LD E, H".to_string(),
        93 => "LD E, L".to_string(),
        94 => "LD E, (HL)".to_string(),
        95 => "LD E, A".to_string(),
        96 => "LD H, B".to_string(),
        97 => "LD H, C".to_string(),
        98 => "LD H, D".to_string(),
        99 => "LD H, E".to_string(),
        100 => "LD H, H".to_string(),
        101 => "LD H, L".to_string(),
        102 => "LD H, (HL)".to_string(),
        103 => "LD H, A".to_string(),
        104 => "LD L, B".to_string(),
        105 => "LD L, C".to_string(),
        106 => "LD L, D".to_string(),
        107 => "LD L, E".to_string(),
        108 => "LD L, H".to_string(),
        109 => "LD L, L".to_string(),
        110 => "LD L, (HL)".to_string(),
        111 => "LD L, A".to_string(),
        112 => "LD (HL), B".to_string(),
        113 => "LD (HL), C".to_string(),
        114 => "LD (HL), D".to_string(),
        115 => "LD (HL), E".to_string(),
        116 => "LD (HL), H".to_string(),
        117 => "LD (HL), L".to_string(),
        118 => "HALT ".to_string(),
        119 => "LD (HL), A".to_string(),
        120 => "LD A, B".to_string(),
        121 => "LD A, C".to_string(),
        122 => "LD A, D".to_string(),
        123 => "LD A, E".to_string(),
        124 => "LD A, H".to_string(),
        125 => "LD A, L".to_string(),
        126 => "LD A, (HL)".to_string(),
        127 => "LD A, A".to_string(),
        128 => "ADD A, B".to_string(),
        129 => "ADD A, C".to_string(),
        130 => "ADD A, D".to_string(),
        131 => "ADD A, E".to_string(),
        132 => "ADD A, H".to_string(),
        133 => "ADD A, L".to_string(),
        134 => "ADD A, (HL)".to_string(),
        135 => "ADD A, A".to_string(),
        136 => "ADC A, B".to_string(),
        137 => "ADC A, C".to_string(),
        138 => "ADC A, D".to_string(),
        139 => "ADC A, E".to_string(),
        140 => "ADC A, H".to_string(),
        141 => "ADC A, L".to_string(),
        142 => "ADC A, (HL)".to_string(),
        143 => "ADC A, A".to_string(),
        144 => "SUB B".to_string(),
        145 => "SUB C".to_string(),
        146 => "SUB D".to_string(),
        147 => "SUB E".to_string(),
        148 => "SUB H".to_string(),
        149 => "SUB L".to_string(),
        150 => "SUB (HL)".to_string(),
        151 => "SUB A".to_string(),
        152 => "SBC A, B".to_string(),
        153 => "SBC A, C".to_string(),
        154 => "SBC A, D".to_string(),
        155 => "SBC A, E".to_string(),
        156 => "SBC A, H".to_string(),
        157 => "SBC A, L".to_string(),
        158 => "SBC A, (HL)".to_string(),
        159 => "SBC A, A".to_string(),
        160 => "AND B".to_string(),
        161 => "AND C".to_string(),
        162 => "AND D".to_string(),
        163 => "AND E".to_string(),
        164 => "AND H".to_string(),
        165 => "AND L".to_string(),
        166 => "AND (HL)".to_string(),
        167 => "AND A".to_string(),
        168 => "XOR B".to_string(),
        169 => "XOR C".to_string(),
        170 => "XOR D".to_string(),
        171 => "XOR E".to_string(),
        172 => "XOR H".to_string(),
        173 => "XOR L".to_string(),
        174 => "XOR (HL)".to_string(),
        175 => "XOR A".to_string(),
        176 => "OR B".to_string(),
        177 => "OR C".to_string(),
        178 => "OR D".to_string(),
        179 => "OR E".to_string(),
        180 => "OR H".to_string(),
        181 => "OR L".to_string(),
        182 => "OR (HL)".to_string(),
        183 => "OR A".to_string(),
        184 => "CP B".to_string(),
        185 => "CP C".to_string(),
        186 => "CP D".to_string(),
        187 => "CP E".to_string(),
        188 => "CP H".to_string(),
        189 => "CP L".to_string(),
        190 => "CP (HL)".to_string(),
        191 => "CP A".to_string(),
        192 => "RET NZ".to_string(),
        193 => "POP BC".to_string(),
        194 => format!("JP NZ, (0x{arg2:02X}{arg1:02X})"),
        195 => format!("JP (0x{arg2:02X}{arg1:02X})"),
        196 => format!("CALL NZ, (0x{arg2:02X}{arg1:02X})"),
        197 => "PUSH BC".to_string(),
        198 => format!("ADD A, 0x{arg1:02X}"),
        199 => "RST 0x00".to_string(),
        200 => "RET Z".to_string(),
        201 => "RET ".to_string(),
        202 => format!("JP Z, (0x{arg2:02X}{arg1:02X})"),
        203 => "PREFIX ".to_string(),
        204 => format!("CALL Z, (0x{arg2:02X}{arg1:02X})"),
        205 => format!("CALL (0x{arg2:02X}{arg1:02X})"),
        206 => format!("ADC A, 0x{arg1:02X}"),
        207 => "RST 0x08".to_string(),
        208 => "RET NC".to_string(),
        209 => "POP DE".to_string(),
        210 => format!("JP NC, (0x{arg2:02X}{arg1:02X})"),
        211 => "ILLEGAL_D3 ".to_string(),
        212 => format!("CALL NC, (0x{arg2:02X}{arg1:02X})"),
        213 => "PUSH DE".to_string(),
        214 => format!("SUB 0x{arg1:02X}"),
        215 => "RST 0x10".to_string(),
        216 => "RET C".to_string(),
        217 => "RETI ".to_string(),
        218 => format!("JP C, (0x{arg2:02X}{arg1:02X})"),
        219 => "ILLEGAL_DB ".to_string(),
        220 => format!("CALL C, (0x{arg2:02X}{arg1:02X})"),
        221 => "ILLEGAL_DD ".to_string(),
        222 => format!("SBC A, 0x{arg1:02X}"),
        223 => "RST 0x18".to_string(),
        224 => format!("LDH (0xFF{arg1:02X}), A"),
        225 => "POP HL".to_string(),
        226 => "LD C, A".to_string(),
        227 => "ILLEGAL_E3 ".to_string(),
        228 => "ILLEGAL_E4 ".to_string(),
        229 => "PUSH HL".to_string(),
        230 => format!("AND 0x{arg1:02X}"),
        231 => "RST 0x20".to_string(),
        232 => format!("ADD SP, 0x{arg1:02X}"),
        233 => "JP HL".to_string(),
        234 => format!("LD (0x{arg2:02X}{arg1:02X}), A"),
        235 => "ILLEGAL_EB ".to_string(),
        236 => "ILLEGAL_EC ".to_string(),
        237 => "ILLEGAL_ED ".to_string(),
        238 => format!("XOR 0x{arg1:02X}"),
        239 => "RST 0x28".to_string(),
        240 => format!("LDH A, (0xFF{arg1:02X})"),
        241 => "POP AF".to_string(),
        242 => "LD A, C".to_string(),
        243 => "DI ".to_string(),
        244 => "ILLEGAL_F4 ".to_string(),
        245 => "PUSH AF".to_string(),
        246 => format!("OR 0x{arg1:02X}"),
        247 => "RST 0x30".to_string(),
        248 => format!("LD HL, SP, 0x{arg1:02X}"),
        249 => "LD SP, HL".to_string(),
        250 => format!("LD A, (0x{arg2:02X}{arg1:02X})"),
        251 => "EI ".to_string(),
        252 => "ILLEGAL_FC ".to_string(),
        253 => "ILLEGAL_FD ".to_string(),
        254 => format!("CP 0x{arg1:02X}"),
        255 => "RST 0x38".to_string(),
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
    paused: bool,

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
            paused: true,
            ..Self::default()
        }
    }

    fn send_command(&self, msg: EmulatorCommand) {
        if let Some(comm_ctx) = self.comm_ctx.as_ref() {
            comm_ctx
                .command_tx
                .send(msg)
                .unwrap_or_else(|_| log::error!("Failed to send message to thread"));
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
                self.send_command(EmulatorCommand::KeyPressed(joypad_key));
            }

            if ctx.input(|i| i.key_released(key)) {
                self.send_command(EmulatorCommand::KeyReleased(joypad_key));
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
                if !self.comm_ctx.is_some() {
                    ui.label(RichText::new("Select a ROM to play").size(20.0).strong());
                    return;
                }

                ui.horizontal(|ui| {
                    if ui.button(if self.paused { "▶" } else { "⏸" }).clicked() {
                        self.paused = !self.paused;
                    }
                    if ui
                        .add_enabled(self.paused, egui::Button::new("Step"))
                        .clicked()
                    {
                        self.send_command(EmulatorCommand::RunUntil(RunUntil::SingleOpcode));
                    }
                    if ui
                        .add_enabled(self.paused, egui::Button::new("Frame"))
                        .clicked()
                    {
                        self.send_command(EmulatorCommand::RunUntil(RunUntil::FrameEnd));
                    }
                    if ui.button("⏹").clicked() {
                        self.send_command(EmulatorCommand::Exit);
                        self.paused = true;
                    }
                });
                ui.separator();

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
                    self.send_command(EmulatorCommand::Exit);
                    self.paused = true;
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
                            self.send_command(EmulatorCommand::Exit);
                            self.paused = true;
                            match spawn(path, ctx) {
                                Ok(comm_ctx) => self.comm_ctx = Some(comm_ctx),
                                Err(err) => log::error!("Failed to load ROM file: {:?}", err),
                            }
                        }
                    }
                });
                if ui.button("Exit").clicked() {
                    self.send_command(EmulatorCommand::Exit);
                }
            });

            // ui.menu_button("Emulation", |_| {});

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

        if !self.paused {
            self.send_command(EmulatorCommand::RunUntil(RunUntil::FrameEnd));
            self.send_command(EmulatorCommand::QueryDebug(self.open_panel));
        }

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
        self.send_command(EmulatorCommand::Exit);
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
enum RunUntil {
    SingleOpcode,
    // PcHit(u16),
    FrameEnd,
}

#[derive(Debug)]
enum EmulatorCommand {
    RunUntil(RunUntil),

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
            save_file_path,
        }
    }

    fn run(&mut self) {
        loop {
            match self.comm_ctx.command_rc.recv() {
                Ok(m) => match m {
                    EmulatorCommand::RunUntil(RunUntil::FrameEnd) => {
                        self.gameboy.run_one_frame();
                        self.gameboy.write_frame(&mut self.comm_ctx.frame_writer);
                        self.send_event(EmulatorEvent::CompletedFrame);
                    }
                    EmulatorCommand::RunUntil(_) => {
                        todo!("Emulator run until commands");
                    }
                    EmulatorCommand::QueryDebug(panel) => self.send_debug_for_panel(panel),
                    EmulatorCommand::KeyPressed(key) => self.gameboy.keydown(key),
                    EmulatorCommand::KeyReleased(key) => self.gameboy.keyup(key),
                    EmulatorCommand::Exit => {
                        match self.gameboy.save(&self.save_file_path) {
                            Ok(msg) => log::info!("{msg}"),
                            Err(err) => log::error!("{err:?}"),
                        }
                        log::info!("Received request to quit. Terminate emulation thread");
                        break;
                    }
                },
                Err(e) => log::error!("{}", e),
            }
        }
    }

    fn send_event(&mut self, event: EmulatorEvent) {
        self.comm_ctx.event_tx.send(event).unwrap_or_else(|_| {
            log::error!("Failed to send emulator event");
        });
    }

    fn send_debug_for_panel(&mut self, panel: Panel) {
        let debug = match panel {
            Panel::Cpu => EmulatorEvent::CpuRegisters(self.gameboy.load_cpu_debug()),
            _ => return,
        };
        self.send_event(debug);
    }
}
