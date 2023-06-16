use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use crate::interrupts::{InterruptHandler, InterruptType};
use crate::memory::Memory;
use crate::palettes::{Palette, RGBA_WHITE};
use crate::{EmulatorEvent, Frame, HardwareSupport, SystemState};

pub(crate) const VRAM_START: u16 = 0x8000;
pub(crate) const VRAM_END: u16 = 0x9FFF;
pub(crate) const VRAM_BANK_SIZE: usize = (VRAM_END - VRAM_START + 1) as usize;

pub(crate) const OAM_START: u16 = 0xFE00;
pub(crate) const OAM_END: u16 = 0xFE9F;
pub(crate) const OAM_SIZE: usize = (OAM_END - OAM_START + 1) as usize;

pub(crate) const PPU_REGISTERS_START: u16 = 0xFF40;
pub(crate) const PPU_REGISTERS_END: u16 = 0xFF4B;

pub(crate) const LCDC_ADDRESS: u16 = 0xFF40;
pub(crate) const STAT_ADDRESS: u16 = 0xFF41;
pub(crate) const SCY_ADDRESS: u16 = 0xFF42;
pub(crate) const SCX_ADDRESS: u16 = 0xFF43;
pub(crate) const LY_ADDRESS: u16 = 0xFF44;
pub(crate) const LYC_ADDRESS: u16 = 0xFF45;
pub(crate) const OAM_DMA_ADDRESS: u16 = 0xFF46;
pub(crate) const BGP_ADDRESS: u16 = 0xFF47;
pub(crate) const OBP0_ADDRESS: u16 = 0xFF48;
pub(crate) const OBP1_ADDRESS: u16 = 0xFF49;
pub(crate) const WY_ADDRESS: u16 = 0xFF4A;
pub(crate) const WX_ADDRESS: u16 = 0xFF4B;
pub(crate) const VRAM_BANK_ADDRESS: u16 = 0xFF4F;

pub(crate) const PALETTE_START: u16 = 0xFF68;
pub(crate) const PALETTE_END: u16 = 0xFF6B;

pub(crate) const BCPS_ADDRESS: u16 = 0xFF68;
pub(crate) const BCPD_ADDRESS: u16 = 0xFF69;
pub(crate) const OCPS_ADDRESS: u16 = 0xFF6A;
pub(crate) const OCPD_ADDRESS: u16 = 0xFF6B;

pub(crate) const DOTS_PER_TICK: u64 = 4;
pub(crate) const OAM_DMA_CYCLES: u64 = 160;

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

#[derive(Debug, Clone, Copy)]
enum SpriteHeight {
    Short = 8,
    Tall = 16,
}

const OAM_SEARCH_DOTS: u64 = 80;
const RENDERING_DOTS: u64 = 168;
const HBLANK_DOTS: u64 = 208;
const SCANLINE_DOTS: u64 = OAM_SEARCH_DOTS + RENDERING_DOTS + HBLANK_DOTS;

pub const LCD_WIDTH: u32 = 160;
pub const LCD_HEIGHT: u32 = 144;

const VBLANK_SCANLINES: u32 = 10;
const TOTAL_SCANLINES: u32 = LCD_HEIGHT + VBLANK_SCANLINES;
const VBLANK_DOTS: u64 = VBLANK_SCANLINES as u64 * SCANLINE_DOTS;

const COLOR_PALETTE_SIZE: usize = 64;

pub(crate) struct Ppu {
    vram: [u8; VRAM_BANK_SIZE * 2],
    vram_bank: usize,

    oam: [u8; OAM_SIZE],

    lcdc: Lcdc,
    stat: LcdStat,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    wy: u8,
    wx: u8,

    // CGB palette registers and data
    bcps: u8,
    ocps: u8,
    color_bg_palettes: [u8; COLOR_PALETTE_SIZE],
    color_obj_palettes: [u8; COLOR_PALETTE_SIZE],

    dots_in_line: u64,
    window_internal_counter: Option<u8>,

    bgp: u8,
    obp0: u8,
    obp1: u8,

    interrupts: Rc<RefCell<InterruptHandler>>,

    frame: Frame,
    event_tx: Sender<EmulatorEvent>,

    system_state: Rc<RefCell<SystemState>>,
}

impl Ppu {
    pub fn new(
        frame: Frame,
        interrupts: Rc<RefCell<InterruptHandler>>,
        system_state: Rc<RefCell<SystemState>>,
        event_tx: Sender<EmulatorEvent>,
    ) -> Self {
        let mut stat: LcdStat = Default::default();
        stat.set_mode(LcdStatus::OamSearch);

        Ppu {
            vram: [0xFF; VRAM_BANK_SIZE * 2],
            vram_bank: 0xFE, // Bank 0. All other bits are 1
            oam: [0xFF; OAM_SIZE],
            lcdc: Default::default(),
            stat,
            dots_in_line: Default::default(),
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            wy: 0x00,
            wx: 0x00,
            window_internal_counter: None,
            bgp: 0x00,
            obp0: 0x00,
            obp1: 0x00,
            bcps: 0x00,
            ocps: 0x00,
            color_bg_palettes: [0xFF; COLOR_PALETTE_SIZE],
            color_obj_palettes: [0xFF; COLOR_PALETTE_SIZE],
            interrupts,
            frame,
            event_tx,
            system_state,
        }
    }

    pub fn tick(&mut self, speed_divider: u64) {
        // Tick 4 times if single speed mode and 2 times if double speed mode
        // The LCD controller speed does not change with the speed mode
        // TODO: Do this only for CGB
        let cycles_to_tick = DOTS_PER_TICK / speed_divider;
        for _ in 0..cycles_to_tick {
            self.dots_in_line += 1;

            match self.stat.mode() {
                LcdStatus::OamSearch if self.dots_in_line == OAM_SEARCH_DOTS => {
                    let old_stat = self.stat;
                    self.stat.set_mode(LcdStatus::Rendering);

                    if !old_stat.is_stat_irq_asserted() && self.stat.is_stat_irq_asserted() {
                        self.interrupts
                            .borrow_mut()
                            .request_interrupt(InterruptType::LcdStat);
                    }
                }
                LcdStatus::Rendering if self.dots_in_line == RENDERING_DOTS => {
                    self.render_line();
                    let old_stat = self.stat;
                    self.stat.set_mode(LcdStatus::Hblank);

                    if !old_stat.is_stat_irq_asserted() && self.stat.is_stat_irq_asserted() {
                        self.interrupts
                            .borrow_mut()
                            .request_interrupt(InterruptType::LcdStat);
                    }
                }
                LcdStatus::Hblank if self.dots_in_line == SCANLINE_DOTS => {
                    self.ly += 1;
                    self.dots_in_line = 0;
                    let old_stat = self.stat;

                    let next_mode = if self.ly == LCD_HEIGHT as u8 {
                        // Going into VBlank
                        self.window_internal_counter = None;

                        if !old_stat.is_stat_irq_asserted()
                            && self
                                .stat
                                .is_stat_interrupt_source_enabled(LcdStatSource::Mode1Vblank)
                        {
                            self.interrupts
                                .borrow_mut()
                                .request_interrupt(InterruptType::LcdStat);
                        }

                        self.interrupts
                            .borrow_mut()
                            .request_interrupt(InterruptType::Vblank);
                        self.event_tx.send(EmulatorEvent::CompletedFrame).unwrap();

                        LcdStatus::Vblank
                    } else {
                        // Going into another LCD line
                        LcdStatus::OamSearch
                    };

                    self.stat.set_mode(next_mode);
                    if !old_stat.is_stat_irq_asserted() && self.stat.is_stat_irq_asserted() {
                        self.interrupts
                            .borrow_mut()
                            .request_interrupt(InterruptType::LcdStat);
                    }

                    // The LY-LYC compare interrupt is actually delayed by 1 CPU cycle
                    let old_stat = self.stat;
                    self.stat.set_ly_lyc_state(self.ly == self.lyc);
                    if !old_stat.is_stat_irq_asserted() && self.stat.is_stat_irq_asserted() {
                        self.interrupts
                            .borrow_mut()
                            .request_interrupt(InterruptType::LcdStat);
                    }
                }
                LcdStatus::Vblank if self.ly == 153 && self.dots_in_line == 8 => {
                    self.ly = 0;

                    let old_stat = self.stat;
                    self.stat.set_ly_lyc_state(self.ly == self.lyc);
                    if !old_stat.is_stat_irq_asserted() && self.stat.is_stat_irq_asserted() {
                        self.interrupts
                            .borrow_mut()
                            .request_interrupt(InterruptType::LcdStat);
                    }
                }
                LcdStatus::Vblank if self.ly == 0 && self.dots_in_line == SCANLINE_DOTS => {
                    let old_stat = self.stat;
                    self.stat.set_mode(LcdStatus::OamSearch);
                    self.dots_in_line = 0;

                    self.stat.set_ly_lyc_state(self.ly == self.lyc);
                    if !old_stat.is_stat_irq_asserted() && self.stat.is_stat_irq_asserted() {
                        self.interrupts
                            .borrow_mut()
                            .request_interrupt(InterruptType::LcdStat);
                    }
                }
                LcdStatus::Vblank if self.dots_in_line == SCANLINE_DOTS => {
                    self.ly += 1;
                    self.dots_in_line = 0;

                    let old_stat = self.stat;
                    self.stat.set_ly_lyc_state(self.ly == self.lyc);
                    if !old_stat.is_stat_irq_asserted() && self.stat.is_stat_irq_asserted() {
                        self.interrupts
                            .borrow_mut()
                            .request_interrupt(InterruptType::LcdStat);
                    }
                }
                _ => {}
            }
        }
    }

    fn render_line(&mut self) {
        if !self.lcdc.lcd_enabled() {
            return;
        }

        self.render_background_line();

        // if self.lcdc.window_enabled() {
        //     self.render_window_line();
        // }

        // if self.lcdc.sprites_enabled() {
        //     self.draw_sprites_on_ly();
        // }
    }

    fn render_background_line(&mut self) {
        let tileset_address = self.lcdc.bg_and_window_tiledata_area() as usize;
        let tilemap_address = self.lcdc.bg_tilemap_area() as usize;

        let screen_y = self.ly as usize;
        let row_first_index = screen_y * LCD_WIDTH as usize * 4;

        let mut frame = self.frame.lock().unwrap();
        let framebuffer = frame.as_mut_slice();

        for screen_x in 0..LCD_WIDTH as usize {
            // Displace the coordinate in the background map by the position of the viewport that is
            // shown on the screen and wrap around the BG map if it overflows the BG map
            let bg_map_x = (screen_x + self.scx as usize) % BG_MAP_SIZE;
            let bg_map_y = (screen_y + self.scy as usize) % BG_MAP_SIZE;

            let tile_x = bg_map_x / TILE_WIDTH_PX;
            let tile_y = bg_map_y / TILE_HEIGHT_PX;

            let tile_index = tile_y * TILES_PER_LINE + tile_x;
            let tile_index_address = tilemap_address + tile_index;

            let tile_id = self.vram[vram_index(tile_index_address as u16, 0)];
            let tile_attr = self.vram[vram_index(tile_index_address as u16, 1)];

            let hardware_support = self.system_state.borrow().hardware_support;
            let bootrom_mapped = self.system_state.borrow().bootrom_mapped;
            // If the bootrom is mapped, run in CGB mode regardless of cart
            let pixel_color = if hardware_support == HardwareSupport::DmgCompat && !bootrom_mapped {
                self.render_dmg_compat_bg(bg_map_x, bg_map_y, tile_id, tileset_address)
            } else {
                self.render_cgb_bg(bg_map_x, bg_map_y, tile_id, tile_attr, tileset_address)
            };

            let pixel_start_index = row_first_index + screen_x * 4;
            let pixel = &mut framebuffer[pixel_start_index..(pixel_start_index + 4)];

            pixel.copy_from_slice(&pixel_color);
        }
    }

    fn render_dmg_compat_bg(
        &self,
        bg_map_x: usize,
        bg_map_y: usize,
        tile_id: u8,
        tileset_address: usize,
    ) -> [u8; 4] {
        let tile_pixel_x = bg_map_x % TILE_WIDTH_PX;
        let tile_pixel_y = bg_map_y % TILE_HEIGHT_PX;

        // DMG compat games always use the first palette
        let palette_spec = &self.color_bg_palettes[0..8];
        let palette = Palette::new_color(palette_spec);

        let tiledata_mem_offset = match self.lcdc.bg_and_window_tiledata_area() {
            TiledataAddressingMode::Signed => (tile_id as i8 as i16 + 128) as usize * SIZEOF_TILE,
            TiledataAddressingMode::Unsigned => tile_id as usize * SIZEOF_TILE,
        };
        let tiledata_line_offset = tile_pixel_y * 2;
        let tile_line_data_start_address =
            tileset_address + tiledata_mem_offset + tiledata_line_offset;

        let pixel_1 = self.vram[vram_index(tile_line_data_start_address as u16, 0)];
        let pixel_2 = self.vram[vram_index(tile_line_data_start_address as u16 + 1, 0)];

        let index = 7 - tile_pixel_x as u8;
        let color_id =
            ((u8::from(pixel_2 & (1 << index) != 0)) << 1) | u8::from(pixel_1 & (1 << index) != 0);
        palette.actual_color_from_index(color_id)
    }

    fn render_cgb_bg(
        &self,
        bg_map_x: usize,
        bg_map_y: usize,
        tile_id: u8,
        tile_attr: u8,
        tileset_address: usize,
    ) -> [u8; 4] {
        let tile_data_vram_bank = ((tile_attr & 8) >> 3) as usize;

        // Vertical flip
        let tile_pixel_y = if (tile_attr & 0x40) != 0 {
            TILE_HEIGHT_PX - (bg_map_y % TILE_HEIGHT_PX) - 1
        } else {
            bg_map_y % TILE_HEIGHT_PX
        };

        // Horizontal flip
        let tile_pixel_x = if (tile_attr & 0x20) != 0 {
            TILE_WIDTH_PX - (bg_map_x % TILE_WIDTH_PX) - 1
        } else {
            bg_map_x % TILE_WIDTH_PX
        };

        let bg_palette_number = tile_attr as usize & 0b111;
        let palette_spec =
            &self.color_bg_palettes[(bg_palette_number * 8)..((bg_palette_number + 1) * 8)];
        let palette = Palette::new_color(palette_spec);

        let tiledata_mem_offset = match self.lcdc.bg_and_window_tiledata_area() {
            TiledataAddressingMode::Signed => (tile_id as i8 as i16 + 128) as usize * SIZEOF_TILE,
            TiledataAddressingMode::Unsigned => tile_id as usize * SIZEOF_TILE,
        };
        let tiledata_line_offset = tile_pixel_y * 2;
        let tile_line_data_start_address =
            tileset_address + tiledata_mem_offset + tiledata_line_offset;

        let pixel_1 =
            self.vram[vram_index(tile_line_data_start_address as u16, tile_data_vram_bank)];
        let pixel_2 =
            self.vram[vram_index(tile_line_data_start_address as u16 + 1, tile_data_vram_bank)];

        let index = 7 - tile_pixel_x as u8;
        let color_id =
            ((u8::from(pixel_2 & (1 << index) != 0)) << 1) | u8::from(pixel_1 & (1 << index) != 0);
        palette.actual_color_from_index(color_id)
    }

    fn render_window_line(&mut self) {
        let palette = Palette::new_greyscale(self.bgp);

        let tileset_address = self.lcdc.bg_and_window_tiledata_area() as usize;
        let tilemap_address = self.lcdc.window_tilemap_area() as usize;

        // The first row of the window has not been reached yet or the window is placed to the
        // extreme right outside the screen
        if self.ly < self.wy || self.wx as u32 >= LCD_WIDTH + 7 {
            return;
        }
        // This is the value of the internal Window counter in the Game boy hardware for this LY
        let window_y = match self.window_internal_counter {
            Some(x) => {
                self.window_internal_counter = Some(x + 1);
                x
            }
            None => {
                self.window_internal_counter = Some(1);
                0
            }
        } as usize;

        let window_x_start = if self.wx < 7 { 7 - self.wx } else { 0x00 } as usize;
        let screen_x_start = self.wx.saturating_sub(7) as usize;

        let screen_y = self.ly as usize;
        // The window spans from the wx - 7 to the end of the scanline
        let row_first_index = screen_y * LCD_WIDTH as usize * 4 + screen_x_start * 4;
        let row_last_index = (screen_y + 1) * LCD_WIDTH as usize * 4 - 1;

        let mut frame = self.frame.lock().unwrap();
        let framebuffer = frame.as_mut_slice();

        for (window_index_x, pixel) in framebuffer[row_first_index..=row_last_index]
            .chunks_mut(4)
            .enumerate()
        {
            let window_x = window_x_start + window_index_x;

            let tile_x = window_x / TILE_WIDTH_PX;
            let tile_y = window_y / TILE_HEIGHT_PX;

            let tile_pixel_x = window_x % TILE_WIDTH_PX;
            let tile_pixel_y = window_y % TILE_HEIGHT_PX;

            let tile_index = tile_y * TILES_PER_LINE + tile_x;
            let tile_index_address = tilemap_address + tile_index;
            let tile_id = self.vram[vram_index(tile_index_address as u16, 0)];

            let tiledata_mem_offset = match self.lcdc.bg_and_window_tiledata_area() {
                TiledataAddressingMode::Signed => {
                    (tile_id as i8 as i16 + 128) as usize * SIZEOF_TILE
                }
                TiledataAddressingMode::Unsigned => tile_id as usize * SIZEOF_TILE,
            };
            let tiledata_line_offset = tile_pixel_y * 2;
            let tile_line_data_start_address =
                tileset_address + tiledata_mem_offset + tiledata_line_offset;

            let pixel_1 = self.vram[vram_index(tile_line_data_start_address as u16, 0)];
            let pixel_2 = self.vram[vram_index(tile_line_data_start_address as u16 + 1, 0)];

            let index = 7 - tile_pixel_x as u8;
            let color_id = ((u8::from(pixel_2 & (1 << index) != 0)) << 1)
                | u8::from(pixel_1 & (1 << index) != 0);
            pixel.copy_from_slice(&palette.actual_color_from_index(color_id));
        }
    }

    fn draw_sprites_on_ly(&mut self) {
        let sprites = self.get_sprites_on_ly();
        let sprite_height = self.lcdc.sprite_height() as u8;

        let mut frame = self.frame.lock().unwrap();
        let framebuffer = frame.as_mut_slice();

        // On the DMG model the sprite priority is determined by two conditions:
        // 1. The smaller the X-coordinate the higher the priority
        // 2. When the X-coordinate is same, the object located first in the
        //    OAM gets priority
        // On CGB, only the second condition is used
        // Drawing in reverse will handle condition 2. For condition 1, the `get_sprites` function
        // should have sorted the sprites in increasing order of the X-coord if we are DMG mode
        for sprite in sprites.iter().rev() {
            // Sprite is hidden beyond the screen
            if sprite.x == 0 || sprite.x >= 168 {
                continue;
            }

            // Sprites always use the 0x8000 unsigned addressing mode
            let sprite_tile_address = match self.lcdc.sprite_height() {
                SpriteHeight::Short => sprite.tile_index,
                // Bit-0 of tile-index should be ignored for tall sprites
                SpriteHeight::Tall => sprite.tile_index & 0xFE,
            } as u16
                * SIZEOF_TILE as u16
                + TiledataAddressingMode::Unsigned as u16;

            // We offset LY by 16 to ease the following calculations and prevent overflow checks
            let offset_ly = self.ly + 16;

            let sprite_line_offset = if sprite.flip_y() {
                sprite_height - (offset_ly - sprite.y) - 1
            } else {
                offset_ly - sprite.y
            };

            let tile_line_data_start_address =
                sprite_tile_address + (sprite_line_offset as u16 * 2);

            let obj_palette_number = if let ObjectPalette::ColorPalette(value) = sprite.palette() {
                value as usize
            } else {
                0x0
            };
            let palette_spec =
                &self.color_obj_palettes[(obj_palette_number * 8)..((obj_palette_number + 1) * 8)];
            let palette = Palette::new_color(palette_spec);

            let pixel_1 = self.vram[vram_index(tile_line_data_start_address, sprite.vram_bank())];
            let pixel_2 =
                self.vram[vram_index(tile_line_data_start_address + 1, sprite.vram_bank())];

            // The sprite is partially hidden on the left
            let (visible_column_start, visible_column_end, screen_x_start) = if sprite.x < 8 {
                (8 - sprite.x, 7, 0)
            } else if sprite.x > 160 {
                // The sprite is partially hidden on the right
                (0, 168 - sprite.x, sprite.x - 8)
            } else {
                // The sprite is entirely visible
                (0, 7, sprite.x - 8)
            };

            let columns_visible = visible_column_end - visible_column_start + 1;

            let sprite_first_index =
                self.ly as usize * 4 * LCD_WIDTH as usize + screen_x_start as usize * 4;
            let sprite_last_index = (self.ly as usize * 4 * LCD_WIDTH as usize
                + (screen_x_start + columns_visible) as usize * 4)
                - 1;

            for (i, pixel) in framebuffer[sprite_first_index..=sprite_last_index]
                .chunks_mut(4)
                .enumerate()
            {
                let pixel_index = if sprite.flip_x() {
                    visible_column_start + i as u8
                } else {
                    7 - (visible_column_start + i as u8)
                };

                let color_id = (u8::from(pixel_2 & (1 << pixel_index) != 0) << 1)
                    | u8::from(pixel_1 & (1 << pixel_index) != 0);
                // Color ID 00 is transparent for sprites
                if color_id != 0b00 {
                    if sprite.bg_window_over_sprite() {
                        if pixel == RGBA_WHITE {
                            pixel.copy_from_slice(&palette.actual_color_from_index(color_id));
                        }
                    } else {
                        pixel.copy_from_slice(&palette.actual_color_from_index(color_id));
                    };
                }
            }
        }
    }

    fn get_sprites_on_ly(&self) -> Vec<Sprite> {
        let mut sprites = Vec::with_capacity(10);
        let sprite_indices = self.sprites_on_ly();

        for index in sprite_indices {
            let sprite_address = index as u16 * 4;
            let entry = &self.oam[sprite_address as usize..(sprite_address as usize + 4)];
            let sprite = Sprite::new(entry[0], entry[1], entry[2], entry[3]);
            sprites.push(sprite);
        }

        // Sorting by the X coordinate will take care of the first condition for DMG where the
        // sprite with the lower X coordinate has higher priority and is drawn over
        // TODO: This should be skipped when running in CGB mode
        // sprites.sort_by(|sprite1, sprite2| sprite1.x.cmp(&sprite2.x));

        sprites
    }

    fn sprites_on_ly(&self) -> Vec<usize> {
        let mut sprites = Vec::with_capacity(10);

        let sprite_height = self.lcdc.sprite_height() as u8;

        // When scanning the OAM the PPU selects only the first 10 sprites which fall on the current
        // scanline
        for (i, sprite) in self.oam.chunks(4).enumerate() {
            let screen_y_start = sprite[0].saturating_sub(16);
            // TODO: Fix overflow on screen_y_end calculation
            let screen_y_end = (sprite[0] + sprite_height).saturating_sub(16);

            // We don't check if the sprite is hidden below the frame because this function should
            // not be called for those values of LY at all
            if screen_y_start <= self.ly && self.ly < screen_y_end {
                sprites.push(i);
            }
        }

        sprites.truncate(10); // The GameBoy LCD can only show 10 sprites per scanline
        sprites
    }

    fn bcp_read(&self) -> u8 {
        self.color_bg_palettes[(self.bcps & 0x1F) as usize]
    }

    fn bcp_write(&mut self, data: u8) {
        self.color_bg_palettes[(self.bcps & 0x1F) as usize] = data;
        if self.bcps & 0x80 != 0 {
            self.bcps += 1;
            self.bcps &= 0x9F;
        }
    }

    fn ocp_read(&self) -> u8 {
        self.color_obj_palettes[(self.ocps & 0x1F) as usize]
    }

    fn ocp_write(&mut self, data: u8) {
        self.color_obj_palettes[(self.ocps & 0x1F) as usize] = data;
        if self.ocps & 0x80 != 0 {
            self.ocps += 1;
            self.ocps &= 0x9F;
        }
    }
}

impl Memory for Ppu {
    fn read(&self, address: u16) -> u8 {
        match address {
            // TODO: VRAM/OAM disable access to CPU after timings are perfect
            VRAM_START..=VRAM_END => self.vram[vram_index(address, self.vram_bank & 0b1)],
            OAM_START..=OAM_END => self.oam[(address - OAM_START) as usize],
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
            VRAM_BANK_ADDRESS => self.vram_bank as u8,
            BCPS_ADDRESS => self.bcps,
            BCPD_ADDRESS => self.bcp_read(),
            OCPS_ADDRESS => self.ocps,
            OCPD_ADDRESS => self.ocp_read(),
            _ => 0xFF,
        }
    }

    fn write(&mut self, address: u16, data: u8) {
        match address {
            VRAM_START..=VRAM_END => self.vram[vram_index(address, self.vram_bank & 0b1)] = data,
            OAM_START..=OAM_END => self.oam[(address - OAM_START) as usize] = data,
            LCDC_ADDRESS => {
                if ((self.lcdc.0 & 0x80) == 0x80) && ((data & 0x80) == 0x00) {
                    self.stat.set_mode(LcdStatus::Hblank);
                    self.ly = 0x00;
                }
                self.lcdc.0 = data;
            }
            // Ignore bit 7 as it is not used and don't set status or lyc=ly on write
            STAT_ADDRESS => self.stat.0 = ((data & 0x78) | (self.stat.0 & 0x7)) & 0x7F,
            SCY_ADDRESS => self.scy = data,
            SCX_ADDRESS => self.scx = data,
            LY_ADDRESS => {}
            LYC_ADDRESS => {
                self.lyc = data;
                self.stat.set_ly_lyc_state(self.ly == self.lyc);
            }
            BGP_ADDRESS => self.bgp = data,
            OBP0_ADDRESS => self.obp0 = data,
            OBP1_ADDRESS => self.obp1 = data,
            WY_ADDRESS => self.wy = data,
            WX_ADDRESS => self.wx = data,
            VRAM_BANK_ADDRESS => self.vram_bank = 0xFE | (data as usize & 0b1),
            BCPS_ADDRESS => self.bcps = data & !(0x1 << 6), // Ignore bit 6
            BCPD_ADDRESS => self.bcp_write(data),
            OCPS_ADDRESS => self.ocps = data & !(0x1 << 6), // Ignore bit 6
            OCPD_ADDRESS => self.ocp_write(data),
            _ => {}
        }
    }
}

fn vram_index(address: u16, bank: usize) -> usize {
    VRAM_BANK_SIZE * bank + (address - VRAM_START) as usize
}

// OAM Sprites
struct Sprite {
    y: u8,
    x: u8,
    tile_index: u8,
    attrs: u8,
}

enum ObjectPalette {
    Obp0,
    Obp1,
    ColorPalette(u8),
}

impl Sprite {
    pub fn new(y: u8, x: u8, tile_index: u8, attrs: u8) -> Self {
        Self {
            y,
            x,
            tile_index,
            attrs,
        }
    }

    pub fn bg_window_over_sprite(&self) -> bool {
        self.attrs & 0x80 != 0
    }

    pub fn flip_y(&self) -> bool {
        self.attrs & 0x40 != 0
    }

    pub fn flip_x(&self) -> bool {
        self.attrs & 0x20 != 0
    }

    pub fn vram_bank(&self) -> usize {
        ((self.attrs & 0x8) >> 3) as usize
    }

    pub fn palette(&self) -> ObjectPalette {
        // Only run this if running in DMG-mode
        // if self.attrs & 0x10 != 0 {
        //     ObjectPalette::Obp1
        // } else {
        //     ObjectPalette::Obp0
        // }
        ObjectPalette::ColorPalette(self.attrs & 0b111)
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
struct Lcdc(u8);

impl Lcdc {
    pub fn lcd_enabled(&self) -> bool {
        (self.0 & LcdcFlags::PpuEnabled as u8) != 0x00
    }

    pub fn window_tilemap_area(&self) -> TilemapBase {
        if self.0 & LcdcFlags::WindowTilemapArea as u8 == 0x00 {
            TilemapBase::Base1
        } else {
            TilemapBase::Base2
        }
    }

    pub fn window_enabled(&self) -> bool {
        (self.0 & LcdcFlags::WindowEnabled as u8) != 0x00
    }

    pub fn bg_and_window_tiledata_area(&self) -> TiledataAddressingMode {
        if self.0 & LcdcFlags::BgAndWindowTileDataArea as u8 == 0x00 {
            TiledataAddressingMode::Signed
        } else {
            TiledataAddressingMode::Unsigned
        }
    }

    pub fn bg_tilemap_area(&self) -> TilemapBase {
        if self.0 & LcdcFlags::BgTilemapArea as u8 == 0x00 {
            TilemapBase::Base1
        } else {
            TilemapBase::Base2
        }
    }

    pub fn sprite_height(&self) -> SpriteHeight {
        if self.0 & LcdcFlags::ObjSize as u8 == 0x00 {
            SpriteHeight::Short
        } else {
            SpriteHeight::Tall
        }
    }

    pub fn sprites_enabled(&self) -> bool {
        self.0 & LcdcFlags::ObjEnabled as u8 != 0x00
    }

    pub fn bg_and_window_enabled(&self) -> bool {
        self.0 & LcdcFlags::BgAndWindowEnabled as u8 != 0x00
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

const LYC_LY_EQUAL: u8 = 1 << 2;
const LCD_STAT_MASK: u8 = 0x03; // Bits 1,0

#[derive(Debug, Default, Copy, Clone)]
struct LcdStat(u8);

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
        self.0 = (self.0 & !LCD_STAT_MASK) | (mode as u8);
    }

    fn is_stat_interrupt_source_enabled(&self, source: LcdStatSource) -> bool {
        self.0 & source as u8 != 0
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

    fn is_stat_irq_asserted(&self) -> bool {
        if self.is_stat_interrupt_source_enabled(LcdStatSource::LycLyEqual) && self.lyc_ly_equal() {
            return true;
        }

        match self.mode() {
            LcdStatus::Hblank => self.is_stat_interrupt_source_enabled(LcdStatSource::Mode0Hblank),
            LcdStatus::Vblank => self.is_stat_interrupt_source_enabled(LcdStatSource::Mode1Vblank),
            LcdStatus::OamSearch => self.is_stat_interrupt_source_enabled(LcdStatSource::Mode2Oam),
            LcdStatus::Rendering => false,
        }
    }
}
