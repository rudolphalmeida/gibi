use std::io;

use crate::{
    memory::Memory,
    options::Options,
    utils::{Byte, Word},
};

pub(crate) trait Cartridge: Memory + Mbc {}

const CARTRIDGE_TYPE_ADDRESS: Word = 0x147;
const ROM_SIZE_ADDRESS: Word = 0x148;
const ROM_BANK_SIZE: u32 = 1024 * 16;
// 16KiB
const RAM_SIZE_ADDRESS: Word = 0x149;
const RAM_BANK_SIZE: u32 = 1024 * 8; // 8KiB

pub const BOOT_ROM_START: Word = 0x0000;
pub const BOOT_ROM_END: Word = 0x00FF;
pub const BOOT_ROM: &'static [Byte; 256] = include_bytes!("../roms/dmg_boot.bin");

pub const CART_ROM_START: Word = 0x0000;
pub const CART_ROM_END: Word = 0x7FFF;

pub const CART_RAM_START: Word = 0xA000;
pub const CART_RAM_END: Word = 0xBFFF;

pub(crate) fn load_from_file(options: &Options) -> io::Result<Box<dyn Cartridge>> {
    let rom = std::fs::read(options.rom_file.as_str())?;
    let mbc = init_mbc_from_rom(rom);

    Ok(mbc)
}

fn init_mbc_from_rom(rom: Vec<Byte>) -> Box<dyn Cartridge> {
    match rom[CARTRIDGE_TYPE_ADDRESS as usize] {
        0x00 => Box::new(NoMbc::new(rom)),
        // TODO: More MBCs
        // TODO: Remove `panic`s
        _ => panic!("Unsupported MBC type"),
    }
}

pub trait Mbc {
    /// Name of the MBC as determined by the cartridge type
    fn name(&self) -> String;

    /// ROM size in bytes
    fn rom_size(&self) -> u32;
    /// Number of ROM banks
    fn rom_banks(&self) -> u32;

    /// RAM size in bytes. 0 if None
    fn ram_size(&self) -> u32;
    /// Number of RAM banks
    fn ram_banks(&self) -> u32;
}

// Memory Banking Controllers (MBCS)
// ROM Only MBC

struct NoMbc {
    rom: Vec<u8>,
}

impl NoMbc {
    pub fn new(rom: Vec<Byte>) -> Self {
        NoMbc { rom }
    }
}

impl Mbc for NoMbc {
    fn name(&self) -> String {
        "ROM ONLY".into()
    }

    fn rom_size(&self) -> u32 {
        rom_size(self.rom[ROM_SIZE_ADDRESS as usize]).0
    }

    fn rom_banks(&self) -> u32 {
        rom_size(self.rom[ROM_SIZE_ADDRESS as usize]).1
    }

    fn ram_size(&self) -> u32 {
        ram_size(self.rom[RAM_SIZE_ADDRESS as usize]).0
    }

    fn ram_banks(&self) -> u32 {
        ram_size(self.rom[RAM_SIZE_ADDRESS as usize]).1
    }
}

impl Memory for NoMbc {
    fn read(&self, address: Word) -> Byte {
        match address {
            0x0000..=0x7FFF => self.rom[address as usize],
            _ => panic!("Read from {:#6X} for {} MBC", address, self.name()),
        }
    }

    fn write(&mut self, address: Word, data: Byte) {
        panic!(
            "Write to {:#6X} with {:#4X} for {} MBC",
            address,
            data,
            self.name()
        );
    }
}

impl Cartridge for NoMbc {}

// Helper methods
/// Calculate the ROM size and number of ROM banks of the cartridge from the
/// byte at 0x148. Return this information as a (size, banks) tuple
fn rom_size(value: Byte) -> (u32, u32) {
    // According to Pan Docs no ROMs with the value 0x52, 0x53, 0x54 exist for
    // any game. So we safely ignore those
    if value > 0x08 {
        panic!("Unknown ROM size byte {:#4X}", value);
    }

    // Calculated as (32KiB << `value`)
    let size = (ROM_BANK_SIZE * 2) << value;
    let banks = size / ROM_BANK_SIZE;

    (size, banks)
}

/// Calculate the RAM size and the number of RAM banks of the cartridge from
/// the byte at 0x149. Return this information as a (size, banks) tuple
fn ram_size(value: Byte) -> (u32, u32) {
    match value {
        0x00 => (0x00, 0x00),             // No RAM
        0x02 => (RAM_BANK_SIZE, 1),       // 8KB
        0x03 => (RAM_BANK_SIZE * 4, 4),   // 32KB
        0x04 => (RAM_BANK_SIZE * 16, 16), // 128 KB
        0x05 => (RAM_BANK_SIZE * 8, 8),   // 64KB
        _ => panic!("Unknown RAM size byte {:#4X}", value),
    }
}
