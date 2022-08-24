use std::cell::RefCell;
use std::rc::Rc;

use crate::interrupts::{InterruptHandler, InterruptType};
use crate::utils::{bit_value, Cycles};
use crate::{
    memory::Memory,
    utils::{Byte, Word},
};

pub(crate) const VRAM_START: Word = 0x8000;
pub(crate) const VRAM_END: Word = 0x9FFF;
pub(crate) const VRAM_SIZE: usize = (VRAM_END - VRAM_START + 1) as usize;

pub(crate) const OAM_START: Word = 0xFE00;
pub(crate) const OAM_END: Word = 0xFE9F;
pub(crate) const OAM_SIZE: usize = (OAM_END - OAM_START + 1) as usize;

pub(crate) const PPU_REGISTERS_START: Word = 0xFF40;
pub(crate) const PPU_REGISTERS_END: Word = 0xFF4B;

pub(crate) const PALETTE_START: Word = 0xFF68;
pub(crate) const PALETTE_END: Word = 0xFF69;

pub(crate) const LCDC_ADDRESS: Word = 0xFF40;
pub(crate) const STAT_ADDRESS: Word = 0xFF41;
pub(crate) const SCY_ADDRESS: Word = 0xFF42;
pub(crate) const SCX_ADDRESS: Word = 0xFF43;
pub(crate) const LY_ADDRESS: Word = 0xFF44;
pub(crate) const LYC_ADDRESS: Word = 0xFF45;
pub(crate) const BGP_ADDRESS: Word = 0xFF47;
pub(crate) const OBP0_ADDRESS: Word = 0xFF48;
pub(crate) const OBP1_ADDRESS: Word = 0xFF49;
pub(crate) const WY_ADDRESS: Word = 0xFF4A;
pub(crate) const WX_ADDRESS: Word = 0xFF4B;

pub(crate) const DOTS_PER_TICK: i32 = 4;

const BG_MAP_SIZE: usize = 256;
const TILE_WIDTH_PX: usize = 8;
const TILE_HEIGHT_PX: usize = 8;
const TILES_PER_LINE: usize = 32;
const SIZEOF_TILE: usize = 16; // Each tile is 16 bytes

enum TilemapBase {
    Base1 = 0x9800,
    Base2 = 0x9C00,
}

const TILEMAP_AREA_SIZE: usize = TilemapBase::Base2 as usize - TilemapBase::Base1 as usize;

enum TiledataAddressingMode {
    Signed = 0x8800,
    Unsigned = 0x8000,
}

enum SpriteHeight {
    Short = 8,
    Tall = 16,
}

type Dots = Cycles; // Each m-cycle is 4 dots

const OAM_SEARCH_DOTS: Dots = 80;
const RENDERING_DOTS: Dots = 168;
const HBLANK_DOTS: Dots = 208;
const SCANLINE_DOTS: Dots = OAM_SEARCH_DOTS + RENDERING_DOTS + HBLANK_DOTS;

pub const LCD_WIDTH: u32 = 160;
pub const LCD_HEIGHT: u32 = 144;

const VBLANK_SCANLINES: u32 = 10;
const TOTAL_SCANLINES: u32 = LCD_HEIGHT + VBLANK_SCANLINES;
const VBLANK_DOTS: Dots = VBLANK_SCANLINES as Dots * SCANLINE_DOTS;

pub(crate) struct Ppu {
    vram: Box<[Byte; VRAM_SIZE]>,
    oam: Box<[Byte; OAM_SIZE]>,
    lcdc: Lcdc,
    stat: LcdStat,
    dots_in_line: Dots,

    scy: Byte,
    scx: Byte,
    ly: Byte,
    lyc: Byte,
    wy: Byte,
    wx: Byte,

    bgp: Byte,
    obp0: Byte,
    obp1: Byte,

    interrupts: Rc<RefCell<InterruptHandler>>,

    framebuffer: Vec<Byte>,
}

impl Ppu {
    pub fn new(interrupts: Rc<RefCell<InterruptHandler>>) -> Self {
        let vram = Box::new([0xFF; VRAM_SIZE]);
        let oam = Box::new([0xFF; OAM_SIZE]);
        let lcdc = Default::default();
        let mut stat: LcdStat = Default::default();
        stat.set_mode(LcdStatus::OamSearch);
        let dots_in_line = Default::default();
        // We are using a RGBA format pixel buffer
        let framebuffer = Vec::from([0x00; (LCD_WIDTH * LCD_HEIGHT * 4) as usize]);

        Ppu {
            vram,
            oam,
            lcdc,
            stat,
            dots_in_line,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            wy: 0x00,
            wx: 0x00,
            bgp: 0x00,
            obp0: 0x00,
            obp1: 0x00,
            interrupts,
            framebuffer,
        }
    }

    pub fn tick(&mut self) {
        for _ in 0..DOTS_PER_TICK {
            self.dots_in_line += 1;

            if self.stat.mode() == LcdStatus::Vblank {
                if self.dots_in_line == SCANLINE_DOTS {
                    self.ly += 1;
                    self.dots_in_line = 0x00;

                    if self.ly == TOTAL_SCANLINES as Byte {
                        self.ly = 0x00;
                        self.stat.set_mode(LcdStatus::OamSearch);

                        if self
                            .stat
                            .is_stat_interrupt_source_enabled(LcdStatSource::Mode2Oam)
                        {
                            self.interrupts
                                .borrow_mut()
                                .request_interrupt(InterruptType::LcdStat);
                        }
                    }
                }
            } else {
                // In Mode 2
                if self.dots_in_line == OAM_SEARCH_DOTS {
                    self.stat.set_mode(LcdStatus::Rendering);
                } else if self.dots_in_line == RENDERING_DOTS {
                    self.stat.set_mode(LcdStatus::Hblank);
                    if self
                        .stat
                        .is_stat_interrupt_source_enabled(LcdStatSource::Mode0Hblank)
                    {
                        self.interrupts
                            .borrow_mut()
                            .request_interrupt(InterruptType::LcdStat);
                    }
                } else if self.dots_in_line == SCANLINE_DOTS {
                    self.render_line();

                    self.ly += 1;
                    self.dots_in_line = 0;

                    self.stat.set_ly_lyc_state(self.ly == self.lyc);

                    if self.stat.lyc_ly_equal()
                        && self
                            .stat
                            .is_stat_interrupt_source_enabled(LcdStatSource::LycLyEqual)
                    {
                        self.interrupts
                            .borrow_mut()
                            .request_interrupt(InterruptType::LcdStat);
                    }

                    if self.ly == LCD_HEIGHT as Byte {
                        self.stat.set_mode(LcdStatus::Vblank);
                        if self
                            .stat
                            .is_stat_interrupt_source_enabled(LcdStatSource::Mode1Vblank)
                        {
                            self.interrupts
                                .borrow_mut()
                                .request_interrupt(InterruptType::Vblank);
                        }
                    } else {
                        self.stat.set_mode(LcdStatus::OamSearch);

                        if self
                            .stat
                            .is_stat_interrupt_source_enabled(LcdStatSource::Mode2Oam)
                        {
                            self.interrupts
                                .borrow_mut()
                                .request_interrupt(InterruptType::LcdStat);
                        }
                    }
                }
            }
        }
    }

    pub fn copy_framebuffer_to_draw_target(&self, buffer: &mut [Byte]) {
        buffer.copy_from_slice(self.framebuffer.as_slice());
    }

    fn render_line(&mut self) {
        if !self.lcdc.lcd_enabled() {
            return;
        }

        if self.lcdc.bg_and_window_enabled() {
            self.render_background_line();
        } else {
            let palette = Palette::new(self.bgp);
            let row_first_index = self.ly as usize * LCD_WIDTH as usize * 4;
            let row_last_index = (self.ly + 1) as usize * LCD_WIDTH as usize * 4 - 1;

            for pixel in self.framebuffer[row_first_index..=row_last_index].chunks_mut(4) {
                pixel.copy_from_slice(palette.color0());
            }
        }
    }

    fn render_background_line(&mut self) {
        let palette = Palette::new(self.bgp);

        let tileset_address = self.lcdc.bg_and_window_tiledata_area() as usize;
        let tilemap_address = self.lcdc.bg_tilemap_area() as usize;

        let screen_y = self.ly as usize;
        let row_first_index = screen_y * LCD_WIDTH as usize * 4;
        let row_last_index = (screen_y + 1) * LCD_WIDTH as usize * 4 - 1;

        for (screen_x, pixel) in self.framebuffer[row_first_index..=row_last_index as usize]
            .chunks_mut(4)
            .enumerate()
        {
            // Displace the coordinate in the background map by the position of the viewport that is
            // shown on the screen and wrap around the BG map if it overflows the BG map
            let bg_map_x = (screen_x + self.scx as usize) % BG_MAP_SIZE;
            let bg_map_y = (screen_y + self.scy as usize) % BG_MAP_SIZE;

            let tile_x = bg_map_x / TILE_WIDTH_PX;
            let tile_y = bg_map_y / TILE_HEIGHT_PX;

            let tile_pixel_x = bg_map_x % TILE_WIDTH_PX;
            let tile_pixel_y = bg_map_y % TILE_HEIGHT_PX;

            let tile_index = tile_y * TILES_PER_LINE + tile_x;
            let tile_index_address = tilemap_address + tile_index;
            let tile_id = self.vram[tile_index_address - VRAM_START as usize];

            let tiledata_mem_offset = match self.lcdc.bg_and_window_tiledata_area() {
                TiledataAddressingMode::Signed => {
                    (tile_id as i8 as i16 + 128) as usize * SIZEOF_TILE
                }
                TiledataAddressingMode::Unsigned => tile_id as usize * SIZEOF_TILE,
            };
            let tiledata_line_offset = tile_pixel_y * 2;
            let tile_line_data_start_address =
                tileset_address + tiledata_mem_offset + tiledata_line_offset;

            let pixel_1 = self.vram[tile_line_data_start_address - VRAM_START as usize];
            let pixel_2 = self.vram[tile_line_data_start_address + 1 - VRAM_START as usize];

            let color_id = (bit_value(pixel_2, 7 - tile_pixel_x as Byte) << 1)
                | bit_value(pixel_1, 7 - tile_pixel_x as Byte);
            pixel.copy_from_slice(palette.actual_color_from_index(color_id));
        }
    }
}

impl Memory for Ppu {
    fn read(&self, address: Word) -> Byte {
        match address {
            // TODO: VRAM/OAM disable access to CPU after timings are perfect
            VRAM_START..=VRAM_END /*if self.stat.mode() != LcdStatus::Rendering*/ => {
                self.vram[(address - VRAM_START) as usize]
            }
            OAM_START..=OAM_END
            /*if self.stat.mode() != LcdStatus::OamSearch
                || self.stat.mode() != LcdStatus::Rendering */ =>
                {
                    self.oam[(address - OAM_START) as usize]
                }
            LCDC_ADDRESS => self.lcdc.0,
            STAT_ADDRESS => self.stat.0,
            SCY_ADDRESS => self.scy,
            SCX_ADDRESS => self.scx,
            LY_ADDRESS => self.ly,
            LYC_ADDRESS => self.lyc,
            BGP_ADDRESS => self.bgp,
            OBP0_ADDRESS => self.obp0,
            OBP1_ADDRESS => self.obp1,
            WY_ADDRESS => self.wy,
            WX_ADDRESS => self.wx,
            _ => 0xFF,
        }
    }

    fn write(&mut self, address: Word, data: Byte) {
        match address {
            VRAM_START..=VRAM_END /* if self.stat.mode() != LcdStatus::Rendering */ => {
                self.vram[(address - VRAM_START) as usize] = data
            }
            OAM_START..=OAM_END
            // if self.stat.mode() != LcdStatus::OamSearch
            //     || self.stat.mode() != LcdStatus::Rendering
            =>
                {
                    self.oam[(address - OAM_START) as usize] = data
                }
            LCDC_ADDRESS => self.lcdc.0 = data,
            // Ignore bit 7 as it is not used and don't set status or lyc=ly on write
            STAT_ADDRESS => self.stat.0 = ((data & 0x78) | (self.stat.0 & 0x7)) & 0x7F,
            SCY_ADDRESS => self.scy = data,
            SCX_ADDRESS => self.scx = data,
            LY_ADDRESS => {}
            LYC_ADDRESS => self.lyc = data,
            BGP_ADDRESS => self.bgp = data,
            OBP0_ADDRESS => self.obp0 = data,
            OBP1_ADDRESS => self.obp1 = data,
            WY_ADDRESS => self.wy = data,
            WX_ADDRESS => self.wx = data,
            _ => {}
        }
    }
}

// LCDC Implementation
enum LcdcFlags {
    PpuEnabled = (1 << 7),
    WindowTilemapArea = (1 << 6),
    WindowEnabled = (1 << 5),
    BgAndWindowTileDataArea = (1 << 4),
    BgTilemapArea = (1 << 3),
    ObjSize = (1 << 2),
    ObjEnabled = (1 << 1),
    BgAndWindowEnabled = 1,
}

#[derive(Debug, Default, Copy, Clone)]
struct Lcdc(Byte);

impl Lcdc {
    pub fn lcd_enabled(&self) -> bool {
        (self.0 & LcdcFlags::PpuEnabled as Byte) != 0x00
    }

    pub fn window_tilemap_area(&self) -> TilemapBase {
        if self.0 & LcdcFlags::WindowTilemapArea as Byte == 0x00 {
            TilemapBase::Base1
        } else {
            TilemapBase::Base2
        }
    }

    pub fn window_enabled(&self) -> bool {
        (self.0 & LcdcFlags::WindowEnabled as Byte) != 0x00
    }

    pub fn bg_and_window_tiledata_area(&self) -> TiledataAddressingMode {
        if self.0 & LcdcFlags::BgAndWindowTileDataArea as Byte == 0x00 {
            TiledataAddressingMode::Signed
        } else {
            TiledataAddressingMode::Unsigned
        }
    }

    pub fn bg_tilemap_area(&self) -> TilemapBase {
        if self.0 & LcdcFlags::BgTilemapArea as Byte == 0x00 {
            TilemapBase::Base1
        } else {
            TilemapBase::Base2
        }
    }

    pub fn sprite_height(&self) -> SpriteHeight {
        if self.0 & LcdcFlags::ObjSize as Byte == 0x00 {
            SpriteHeight::Short
        } else {
            SpriteHeight::Tall
        }
    }

    pub fn sprites_enabled(&self) -> bool {
        self.0 & LcdcFlags::ObjEnabled as Byte != 0x00
    }

    pub fn bg_and_window_enabled(&self) -> bool {
        self.0 & LcdcFlags::BgAndWindowEnabled as Byte != 0x00
    }
}

// LCD STAT Implementation
pub(crate) enum LcdStatSource {
    LycLyEqual = (1 << 6),
    Mode2Oam = (1 << 5),
    Mode1Vblank = (1 << 4),
    Mode0Hblank = (1 << 3),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum LcdStatus {
    Hblank = 0,
    Vblank = 1,
    OamSearch = 2,
    Rendering = 3,
}

const LYC_LY_EQUAL: Byte = 1 << 2;
const LCD_STAT_MASK: Byte = 0x03; // Bits 1,0

#[derive(Debug, Default, Copy, Clone)]
struct LcdStat(Byte);

impl LcdStat {
    fn mode(&self) -> LcdStatus {
        match self.0 & LCD_STAT_MASK {
            0 => LcdStatus::Hblank,
            1 => LcdStatus::Vblank,
            2 => LcdStatus::OamSearch,
            3 => LcdStatus::Rendering,
            _ => panic!("Impossible status for LCD mode"),
        }
    }

    fn set_mode(&mut self, mode: LcdStatus) {
        self.0 = (self.0 & !LCD_STAT_MASK) | (mode as Byte);
    }

    fn is_stat_interrupt_source_enabled(&self, source: LcdStatSource) -> bool {
        self.0 & source as Byte != 0
    }

    fn lyc_ly_equal(&self) -> bool {
        self.0 & LYC_LY_EQUAL != 0
    }

    fn set_ly_lyc_state(&mut self, set: bool) {
        if set {
            self.0 |= LYC_LY_EQUAL;
        } else {
            self.0 &= !LYC_LY_EQUAL;
        }
    }
}

enum GameboyColorShade {
    White = 0,
    LightGray = 1,
    DarkGray = 2,
    Black = 3,
}

impl GameboyColorShade {
    pub fn new(bits: Byte) -> Self {
        match bits {
            0 => GameboyColorShade::White,
            1 => GameboyColorShade::LightGray,
            2 => GameboyColorShade::DarkGray,
            3 => GameboyColorShade::Black,
            _ => panic!("Invalid bits for Color shade"),
        }
    }
}

// TODO: Make this configurable by the GUI
const RGBA_WHITE: [Byte; 4] = [0x9B, 0xBC, 0x0F, 0xFF];
const RGBA_LIGHT_GRAY: [Byte; 4] = [0x8B, 0xAC, 0x0F, 0xFF];
const RGBA_DARK_GRAY: [Byte; 4] = [0x30, 0x62, 0x30, 0xFF];
const RGBA_BLACK: [Byte; 4] = [0x0F, 0x38, 0x0F, 0xFF];

fn map_to_actual_color(shade: GameboyColorShade) -> &'static [Byte; 4] {
    match shade {
        GameboyColorShade::White => &RGBA_WHITE,
        GameboyColorShade::LightGray => &RGBA_LIGHT_GRAY,
        GameboyColorShade::DarkGray => &RGBA_DARK_GRAY,
        GameboyColorShade::Black => &RGBA_BLACK,
    }
}

/// Convenience struct to get a color from a palette register
struct Palette(Byte);

impl Palette {
    pub fn new(value: Byte) -> Self {
        Palette(value)
    }

    pub fn actual_color_from_index(&self, index: Byte) -> &[Byte; 4] {
        match index {
            0 => self.color0(),
            1 => self.color1(),
            2 => self.color2(),
            3 => self.color3(),
            _ => panic!("Invalid index for color palette {}", index),
        }
    }

    pub fn color0(&self) -> &[Byte; 4] {
        map_to_actual_color(GameboyColorShade::new(self.0 & 0x03))
    }

    pub fn color1(&self) -> &[Byte; 4] {
        map_to_actual_color(GameboyColorShade::new((self.0 & 0x0C) >> 2))
    }

    pub fn color2(&self) -> &[Byte; 4] {
        map_to_actual_color(GameboyColorShade::new((self.0 & 0x30) >> 4))
    }

    pub fn color3(&self) -> &[Byte; 4] {
        map_to_actual_color(GameboyColorShade::new((self.0 & 0xC0) >> 6))
    }
}
