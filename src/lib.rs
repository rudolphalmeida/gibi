#![allow(dead_code)] // Only for development

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

struct SystemState {
    execution_state: ExecutionState,
    /// Hardware supported by current cartridge
    hardware_support: HardwareSupport,

    key1: u8,
    bootrom_mapped: bool,

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
        ((self.key1 & 0b1) + 1).into()
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
