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

pub(crate) struct Ppu {
    vram: [Byte; VRAM_SIZE],
    oam: [Byte; OAM_SIZE],
    lcdc: Lcdc,
    stat: LcdStat,
}

impl Ppu {
    pub fn new() -> Self {
        let vram = [0xFF; VRAM_SIZE];
        let oam = [0xFF; OAM_SIZE];
        let lcdc = Default::default();
        let stat = Default::default();

        Ppu {
            vram,
            oam,
            lcdc,
            stat,
        }
    }

    pub fn tick(&mut self) {}
}

impl Memory for Ppu {
    fn read(&self, address: Word) -> Byte {
        match address {
            VRAM_START..=VRAM_END => self.vram[(address - VRAM_START) as usize],
            OAM_START..=OAM_END => self.oam[(address - OAM_START) as usize],
            LCDC_ADDRESS => self.lcdc.0,
            STAT_ADDRESS => self.stat.0,
            LY_ADDRESS => 0x90,
            _ => 0x90,
        }
    }

    fn write(&mut self, address: Word, data: Byte) {
        match address {
            VRAM_START..=VRAM_END => self.vram[(address - VRAM_START) as usize] = data,
            OAM_START..=OAM_END => self.oam[(address - OAM_START) as usize] = data,
            LCDC_ADDRESS => self.lcdc.0 = data,
            // Ignore bit 7 as it is not used and don't set status or lyc=ly on write
            STAT_ADDRESS => self.stat.0 = data & !LCD_STAT_MASK & !LYC_LY_EQUAL & 0x7F,
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

    fn lyc_ly_status(&self) -> bool {
        self.0 & LYC_LY_EQUAL != 0
    }

    fn update_lyc_state(&mut self, set: bool) {
        if set {
            self.0 |= LYC_LY_EQUAL;
        } else {
            self.0 &= !LYC_LY_EQUAL;
        }
    }
}
