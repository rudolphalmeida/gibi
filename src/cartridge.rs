use crate::utils::min_number_of_bits;
use crate::{
    memory::Memory,
    utils::{Byte, Word},
};

pub(crate) trait Cartridge: Memory + Mbc {}

const CARTRIDGE_TYPE_ADDRESS: Word = 0x147;
const ROM_SIZE_ADDRESS: Word = 0x148;
const ROM_BANK_SIZE: u32 = 1024 * 16; // 16KiB
const RAM_SIZE_ADDRESS: Word = 0x149;
const RAM_BANK_SIZE: u32 = 1024 * 8; // 8KiB

pub const BOOT_ROM_START: Word = 0x0000;
pub const BOOT_ROM_END: Word = 0x00FF;
pub const BOOT_ROM: &[Byte; 256] = include_bytes!("../roms/dmg_boot.bin");

pub const CART_ROM_START: Word = 0x0000;
pub const CART_ROM_END: Word = 0x7FFF;

pub const CART_RAM_START: Word = 0xA000;
pub const CART_RAM_END: Word = 0xBFFF;

pub(crate) fn init_mbc_from_rom(rom: Vec<Byte>, ram: Option<Vec<Byte>>) -> Box<dyn Cartridge> {
    match rom[CARTRIDGE_TYPE_ADDRESS as usize] {
        0x00 => Box::new(NoMbc::new(rom)),
        0x01 | 0x02 | 0x03 => Box::new(Mbc1::new(rom, ram)),
        // TODO: More MBCs
        // TODO: Remove `panic`s
        _ => panic!(
            "Unsupported MBC type: {:#04X}",
            rom[CARTRIDGE_TYPE_ADDRESS as usize]
        ),
    }
}

pub trait Mbc {
    /// Name of the MBC as determined by the cartridge type
    fn name(&self) -> String;

    fn rom(&self) -> &Vec<Byte>;

    fn ram(&self) -> Option<&Vec<Byte>> {
        None
    }

    /// ROM size in bytes
    fn rom_size(&self) -> u32 {
        rom_size(self.rom()[ROM_SIZE_ADDRESS as usize]).0
    }

    /// Number of ROM banks
    fn rom_banks(&self) -> u32 {
        rom_size(self.rom()[ROM_SIZE_ADDRESS as usize]).1
    }

    /// RAM size in bytes. 0 if None
    fn ram_size(&self) -> u32 {
        ram_size(self.rom()[RAM_SIZE_ADDRESS as usize]).0
    }

    /// Number of RAM banks
    fn ram_banks(&self) -> u32 {
        ram_size(self.rom()[RAM_SIZE_ADDRESS as usize]).1
    }
}

// Memory Banking Controllers (MBCS)
// ROM Only MBC ------------------------------------------------------------------------------------
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

    fn rom(&self) -> &Vec<Byte> {
        &self.rom
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
// End-ROM Only MBC---------------------------------------------------------------------------------

// MBC1 --------------------------------------------------------------------------------------------

struct Mbc1 {
    rom: Vec<Byte>,
    ram: Option<Vec<Byte>>,

    rom_bank: Byte,
    rom_bit_mask: Byte,

    ram_bank: Byte,
    ram_enabled: bool,
    ram_banking_mode: bool,
}

impl Mbc1 {
    pub fn new(rom: Vec<Byte>, mut ram: Option<Vec<Byte>>) -> Self {
        let rom_bank = 0x01;
        let ram_bank = 0x00;
        let ram_enabled = false;
        let ram_banking_mode = false;

        let rom_banks = rom_size(rom[ROM_SIZE_ADDRESS as usize]).1;
        let rom_bits_required = min_number_of_bits(rom_banks as Byte) - 1;
        let rom_bit_mask = (i8::MIN >> (rom_bits_required - 1)) as Byte >> (8 - rom_bits_required);

        let ram_size = ram_size(rom[RAM_SIZE_ADDRESS as usize]).0;
        if ram.is_none() && ram_size > 0 {
            log::info!(
                "No RAM provided. Initializing RAM of size {} bytes",
                ram_size
            );
            ram = Some(vec![0xFF; ram_size as usize]);
        }

        Mbc1 {
            rom,
            ram,
            rom_bank,
            rom_bit_mask,
            ram_bank,
            ram_enabled,
            ram_banking_mode,
        }
    }
}

impl Mbc for Mbc1 {
    fn name(&self) -> String {
        "MBC1".into()
    }

    fn rom(&self) -> &Vec<Byte> {
        &self.rom
    }

    fn ram(&self) -> Option<&Vec<Byte>> {
        self.ram.as_ref()
    }
}

impl Memory for Mbc1 {
    fn read(&self, address: Word) -> Byte {
        match address {
            _ => panic!("Read from {:#6X} for {} MBC", address, self.name()),
        }
    }

    fn write(&mut self, address: Word, data: Byte) {
        match address {
            0x0000..=0x1FFF => self.ram_enabled = data & 0x0F == 0x0A,
            0x2000..=0x3FFF => {
                self.rom_bank = data & 0x1F; // Write the full 5 bits
                if self.rom_bank == 0x00 {
                    self.rom_bank = 0x01;
                }
            }
            0x4000..=0x5FFF => self.ram_bank = data & 0x03,
            0x6000..=0x7FFF => self.ram_banking_mode = data != 0x00,
            // TODO: Write to RAM if enabled and exists
            0xA000..=0xBFFF if self.ram.is_some() && self.ram_enabled && self.ram_banking_mode => {
                if let Some(ram) = self.ram.as_mut() {
                    ram[0xA000 + self.ram_bank as usize * 0x2000] = data;
                }
            }
            0xA000..=0xBFFF => {}
            _ => panic!(
                "Write to {:#6X} with {:#4X} for {} MBC",
                address,
                data,
                self.name()
            ),
        }
    }
}

impl Cartridge for Mbc1 {}

// END-MBC1 ----------------------------------------------------------------------------------------

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
