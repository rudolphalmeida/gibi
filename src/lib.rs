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
