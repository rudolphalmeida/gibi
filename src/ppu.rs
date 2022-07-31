use std::cell::RefCell;
use std::rc::Rc;

use crate::interrupts::{InterruptHandler, InterruptType};
use crate::utils::Cycles;
use crate::{
    memory::Memory,
    utils::{Byte, Word},
};

pub const VRAM_START: Word = 0x8000;
pub const VRAM_END: Word = 0x9FFF;
pub const VRAM_SIZE: usize = (VRAM_END - VRAM_START + 1) as usize;

pub const OAM_START: Word = 0xFE00;
pub const OAM_END: Word = 0xFE9F;
pub const OAM_SIZE: usize = (OAM_END - OAM_START + 1) as usize;

pub const PPU_REGISTERS_START: Word = 0xFF40;
pub const PPU_REGISTERS_END: Word = 0xFF4B;

pub const PALETTE_START: Word = 0xFF68;
pub const PALETTE_END: Word = 0xFF69;

pub const LCDC_ADDRESS: Word = 0xFF40;
pub const STAT_ADDRESS: Word = 0xFF41;
pub const SCY_ADDRESS: Word = 0xFF42;
pub const SCX_ADDRESS: Word = 0xFF43;
pub const LY_ADDRESS: Word = 0xFF44;
pub const LYC_ADDRESS: Word = 0xFF45;
pub const BGP_ADDRESS: Word = 0xFF47;
pub const OBP0_ADDRESS: Word = 0xFF48;
pub const OBP1_ADDRESS: Word = 0xFF49;
pub const WY_ADDRESS: Word = 0xFF4A;
pub const WX_ADDRESS: Word = 0xFF4B;

const DOTS_PER_TICK: i32 = 4;

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

    interrupts: Rc<RefCell<InterruptHandler>>,
}

impl Ppu {
    pub fn new(interrupts: Rc<RefCell<InterruptHandler>>) -> Self {
        let vram = Box::new([0xFF; VRAM_SIZE]);
        let oam = Box::new([0xFF; OAM_SIZE]);
        let lcdc = Default::default();
        let mut stat: LcdStat = Default::default();
        stat.set_mode(LcdStatus::OamSearch);
        let dots_in_line = Default::default();

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
            interrupts,
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
                    }
                }
            }
        }
    }

    fn render_line(&self) {}
}

impl Memory for Ppu {
    fn read(&self, address: Word) -> Byte {
        match address {
            VRAM_START..=VRAM_END if self.stat.mode() != LcdStatus::Rendering => {
                self.vram[(address - VRAM_START) as usize]
            }
            OAM_START..=OAM_END
                if self.stat.mode() != LcdStatus::OamSearch
                    || self.stat.mode() != LcdStatus::Rendering =>
            {
                self.oam[(address - OAM_START) as usize]
            }
            LCDC_ADDRESS => self.lcdc.0,
            STAT_ADDRESS => self.stat.0,
            SCY_ADDRESS => self.scx,
            SCX_ADDRESS => self.scx,
            LY_ADDRESS => self.ly,
            LYC_ADDRESS => self.lyc,
            WY_ADDRESS => self.wy,
            WX_ADDRESS => self.wx,
            _ => 0xFF,
        }
    }

    fn write(&mut self, address: Word, data: Byte) {
        match address {
            VRAM_START..=VRAM_END if self.stat.mode() != LcdStatus::Rendering => {
                self.vram[(address - VRAM_START) as usize] = data
            }
            OAM_START..=OAM_END
                if self.stat.mode() != LcdStatus::OamSearch
                    || self.stat.mode() != LcdStatus::Rendering =>
            {
                self.oam[(address - OAM_START) as usize] = data
            }
            LCDC_ADDRESS => self.lcdc.0 = data,
            // Ignore bit 7 as it is not used and don't set status or lyc=ly on write
            STAT_ADDRESS => self.stat.0 = data & !LCD_STAT_MASK & !LYC_LY_EQUAL & 0x7F,
            SCY_ADDRESS => self.scx = data,
            SCX_ADDRESS => self.scx = data,
            LY_ADDRESS => self.ly = data,
            LYC_ADDRESS => self.lyc = data,
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
    BgAndWindowEnabled = 0,
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
        // TODO: Raise interrupt for LCD STAT after calling this method
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
