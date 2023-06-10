#![allow(dead_code)] // Only for development

use ppu::{LCD_HEIGHT, LCD_WIDTH};

mod apu;
mod cartridge;
mod cpu;
pub mod gameboy;
mod interrupts;
pub mod joypad;
mod memory;
mod mmu;
mod palettes;
pub mod ppu;
mod serial;
mod timer;

pub const GAMEBOY_WIDTH: f32 = LCD_WIDTH as f32;
pub const GAMEBOY_HEIGHT: f32 = LCD_HEIGHT as f32;

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum HardwareSupport {
    CgbOnly,
    DmgCgb,
    DmgCompat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ExecutionState {
    ExecutingProgram,
    PreparingSpeedSwitch,
    Halted,
}

struct HdmaState {
    source_addr: u16, // Built from HDMA1, HDMA2
    dest_addr: u16,   // Built from HDMA3, HDMA4
    hdma_stat: u8,    // HDMA5 (Length, Mode, Start)
}

impl HdmaState {
    pub(crate) fn is_hdma_active(&self) -> bool {
        (self.hdma_stat & 0x80) == 0x00
    }

    fn write_high(attrib: &mut u16, high: u8) {
        *attrib = (*attrib & 0x00FF) | ((high as u16) << 8);
    }

    fn write_low(attrib: &mut u16, low: u8) {
        *attrib = (*attrib & 0xFF00) | (low as u16);
    }

    pub(crate) fn write_src_high(&mut self, high: u8) {
        HdmaState::write_high(&mut self.source_addr, high);
    }

    pub(crate) fn write_src_low(&mut self, low: u8) {
        HdmaState::write_low(&mut self.source_addr, low & 0xF0);
    }

    pub(crate) fn write_dest_high(&mut self, high: u8) {
        HdmaState::write_high(&mut self.dest_addr, high & 0x1F);
    }

    pub(crate) fn write_dest_low(&mut self, low: u8) {
        HdmaState::write_low(&mut self.dest_addr, low & 0xF0);
    }
}

struct SystemState {
    execution_state: ExecutionState,
    /// Hardware supported by current cartridge
    hardware_support: HardwareSupport,

    key1: u8,
    bootrom_mapped: bool,

    hdma_state: HdmaState,

    /// Since we run the CPU one opcode at a time or more, each frame can overrun
    /// the `CYCLES_PER_FRAME` (`17556`) value by a tiny amount. However, eventually
    /// these add up and one frame of CPU execution can miss the PPU frame by a
    /// few scanlines. We use this value to keep track of excess cycles in the
    /// previous frame and ignore those many in the current frame
    carry_over_cycles: u64,
    total_cycles: u64,
}

impl SystemState {
    pub(crate) fn speed_multiplier(&self) -> u64 {
        (((self.key1 & 0x80) >> 7) + 1).into()
    }
}

/// Calculate the minimum number of bits required to store a value
pub(crate) fn min_number_of_bits(mut value: u8) -> u8 {
    let mut count = 0;
    while value > 0 {
        count += 1;
        value >>= 1;
    }

    count
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_min_number_of_bits() {
        assert_eq!(min_number_of_bits(4), 3);
        assert_eq!(min_number_of_bits(5), 3);
        assert_eq!(min_number_of_bits(8), 4);
        assert_eq!(min_number_of_bits(16), 5);
        assert_eq!(min_number_of_bits(32), 6);
        assert_eq!(min_number_of_bits(64), 7);
        assert_eq!(min_number_of_bits(128), 8);
    }
}
