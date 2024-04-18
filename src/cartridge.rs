use crate::{memory::Memory, min_number_of_bits, HardwareSupport};

const CGB_FLAG_ADDRESS: u16 = 0x143;
const CARTRIDGE_TYPE_ADDRESS: u16 = 0x147;
const ROM_SIZE_ADDRESS: u16 = 0x148;
const ROM_BANK_SIZE: u32 = 1024 * 16;
const RAM_SIZE_ADDRESS: u16 = 0x149;
const RAM_BANK_SIZE: u32 = 1024 * 8;

pub const BOOT_ROM_START: u16 = 0x0000;
pub const BOOT_ROM_END: u16 = 0x08FF;
pub const CGB_BOOT_ROM: &[u8; 0x900] = include_bytes!("../roms/cgb_boot.bin");

pub const CART_ROM_START: u16 = 0x0000;
pub const CART_ROM_END: u16 = 0x7FFF;

pub const CART_RAM_START: u16 = 0xA000;
pub const CART_RAM_END: u16 = 0xBFFF;

pub(crate) trait Savable {
    fn savable(&self) -> bool;
    fn save_ram(&self) -> Option<&Vec<u8>>;
}

pub(crate) trait Cartridge: Memory + Mbc + Savable {
    fn hardware_supported(&self) -> HardwareSupport {
        match self.rom()[CGB_FLAG_ADDRESS as usize] {
            0x80 => HardwareSupport::DmgCgb,
            0xC0 => HardwareSupport::CgbOnly,
            _ => HardwareSupport::DmgCompat,
        }
    }
}

pub(crate) fn init_mbc_from_rom(rom: Vec<u8>, ram: Option<Vec<u8>>) -> Box<dyn Cartridge> {
    match rom[CARTRIDGE_TYPE_ADDRESS as usize] {
        0x00 => Box::new(NoMbc::new(rom)),
        x @ (0x01..=0x03) => Box::new(Mbc1::new(rom, ram, x == 0x03)),
        x @ (0x19..=0x1E) => Box::new(Mbc5::new(
            rom,
            ram,
            x == 0x1B || x == 0x1E,
            x == 0x1C || x == 0x1D || x == 0x1E,
        )),
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

    fn rom(&self) -> &Vec<u8>;

    fn ram(&self) -> Option<&Vec<u8>> {
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
    pub fn new(rom: Vec<u8>) -> Self {
        NoMbc { rom }
    }
}

impl Mbc for NoMbc {
    fn name(&self) -> String {
        "ROM ONLY".into()
    }

    fn rom(&self) -> &Vec<u8> {
        &self.rom
    }
}

impl Memory for NoMbc {
    fn read(&mut self, address: u16) -> u8 {
        match address {
            0x0000..=0x7FFF => self.rom[address as usize],
            _ => {
                log::error!("Read from {:#6X} for {} MBC", address, self.name());
                0xFF
            }
        }
    }

    fn write(&mut self, address: u16, data: u8) {
        log::error!(
            "Write to {:#6X} with {:#4X} for {} MBC",
            address,
            data,
            self.name()
        );
    }
}

impl Savable for NoMbc {
    fn savable(&self) -> bool {
        false
    }

    fn save_ram(&self) -> Option<&Vec<u8>> {
        None
    }
}

impl Cartridge for NoMbc {}
// End-ROM Only MBC---------------------------------------------------------------------------------

// MBC1 --------------------------------------------------------------------------------------------

struct Mbc1 {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,

    rom_bank: u8,
    rom_bit_mask: u8,

    ram_bank: u8,
    ram_enabled: bool,
    ram_banking_mode: bool,

    savable: bool,
}

impl Mbc1 {
    pub fn new(rom: Vec<u8>, mut ram: Option<Vec<u8>>, savable: bool) -> Self {
        let rom_bank = 0x01;
        let ram_bank = 0x00;
        let ram_enabled = false;
        let ram_banking_mode = false;

        let rom_banks = rom_size(rom[ROM_SIZE_ADDRESS as usize]).1;
        let rom_bits_required = min_number_of_bits(rom_banks as u8) - 1;
        let rom_bit_mask = (i8::MIN >> (rom_bits_required - 1)) as u8 >> (8 - rom_bits_required);

        let ram_size = ram_size(rom[RAM_SIZE_ADDRESS as usize]).0;
        if ram.is_none() && ram_size > 0 {
            log::info!(
                "No RAM provided. Initializing RAM of size {} bytes",
                ram_size
            );
            ram = Some(vec![0xFF; ram_size as usize]);
        } else if let Some(r) = ram.as_ref() {
            if r.len() != ram_size as usize {
                log::error!(
                    "Provided RAM size {} does not match what was expected {}",
                    r.len(),
                    ram_size
                );
                ram = Some(vec![0xFF; ram_size as usize]);
            }
        }

        Mbc1 {
            rom,
            ram,
            rom_bank,
            rom_bit_mask,
            ram_bank,
            ram_enabled,
            ram_banking_mode,
            savable,
        }
    }

    fn effective_ram_address(&self, address: u16) -> usize {
        if self.ram_banks() > 1 {
            if self.ram_banking_mode {
                0x2000 * self.ram_bank as usize + (address as usize - 0xA000)
            } else {
                // RAM banking not enabled. Use the 0 bank of RAM
                address as usize - 0xA000
            }
        } else {
            // Only one bank of RAM exists either the full 8KB or 2KB (which requires
            // the % RAM_SIZE)
            (address as usize - 0xA000) % self.ram_size() as usize
        }
    }
}

impl Mbc for Mbc1 {
    fn name(&self) -> String {
        "MBC1".into()
    }

    fn rom(&self) -> &Vec<u8> {
        &self.rom
    }

    fn ram(&self) -> Option<&Vec<u8>> {
        self.ram.as_ref()
    }
}

impl Memory for Mbc1 {
    fn read(&mut self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF if !self.ram_banking_mode => self.rom[address as usize],
            0x0000..=0x3FFF if self.ram_banking_mode => {
                let zero_bank_number = if self.rom_banks() <= 32 {
                    0x00
                } else if self.rom_banks() == 64 {
                    (self.ram_bank & 0b1) << 4
                } else {
                    (self.ram_bank << 5) | self.rom_bank
                };

                self.rom[0x4000 * zero_bank_number as usize + address as usize]
            }
            0x4000..=0x7FFF => {
                let high_bank_number = if self.rom_banks() <= 32 {
                    self.rom_bank
                } else if self.rom_banks() == 64 {
                    ((self.ram_bank & 0b1) << 4) | (self.rom_bank)
                } else {
                    (self.ram_bank << 5) | (self.rom_bank)
                };

                self.rom[0x4000 * high_bank_number as usize + (address as usize - 0x4000)]
            }
            0xA000..=0xBFFF if !self.ram_enabled => 0xFF,
            0xA000..=0xBFFF => {
                if let Some(ram) = self.ram.as_ref() {
                    let effective_address = self.effective_ram_address(address);
                    ram[effective_address]
                } else {
                    log::error!(
                        "Read from RAM address {:#6X} for {} MBC with no RAM",
                        address,
                        self.name()
                    );
                    0xFF
                }
            }
            _ => {
                log::error!("Read from {:#6X} for {} MBC", address, self.name());
                0xFF
            }
        }
    }

    fn write(&mut self, address: u16, data: u8) {
        match address {
            0x0000..=0x1FFF => self.ram_enabled = data & 0x0F == 0x0A,
            0x2000..=0x3FFF => {
                let mut rom_bank = data & 0x1F;
                // The full 5 bits are used for the 00->01 translation. This can allow
                // bank 0 to be mapped to 0x4000..=0x7FFF if, for example, the ROM needs only 4 bits but the value
                // 0b10000 is being written here. The non-zero value will prevent the 01 translation
                // however the actual ROM bits will be the lower 4-bits
                if rom_bank == 0x00 {
                    rom_bank = 0x01;
                }

                // However only the actual number of required bits are used for the ROM bank
                // selection
                self.rom_bank = rom_bank & self.rom_bit_mask;
            }
            0x4000..=0x5FFF => self.ram_bank = data & 0x03,
            0x6000..=0x7FFF => self.ram_banking_mode = (data & 0b1) == 0b1,
            0xA000..=0xBFFF if self.ram_enabled => {
                let effective_address = self.effective_ram_address(address);
                if let Some(ram) = self.ram.as_mut() {
                    ram[effective_address] = data;
                }
            }
            0xA000..=0xBFFF => {}
            _ => log::error!(
                "Write to {:#6X} with {:#4X} for {} MBC",
                address,
                data,
                self.name()
            ),
        }
    }
}

impl Savable for Mbc1 {
    fn savable(&self) -> bool {
        self.savable
    }

    fn save_ram(&self) -> Option<&Vec<u8>> {
        self.ram.as_ref()
    }
}

impl Cartridge for Mbc1 {}

// END-MBC1 ----------------------------------------------------------------------------------------

// MBC5 --------------------------------------------------------------------------------------------

struct Mbc5 {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,

    rom_bank: u16,

    ram_bank: u8,
    ram_enabled: bool,

    savable: bool,

    has_rumble: bool,
    rumble_active: bool,
}

impl Mbc5 {
    pub fn new(rom: Vec<u8>, mut ram: Option<Vec<u8>>, savable: bool, has_rumble: bool) -> Self {
        let rom_bank = 0x01;
        let ram_bank = 0x00;
        let ram_enabled = false;
        let rumble_active = false;

        let ram_size = ram_size(rom[RAM_SIZE_ADDRESS as usize]).0;
        if ram.is_none() && ram_size > 0 {
            log::info!(
                "No RAM provided. Initializing RAM of size {} bytes",
                ram_size
            );
            ram = Some(vec![0xFF; ram_size as usize]);
        } else if let Some(r) = ram.as_ref() {
            if r.len() != ram_size as usize {
                log::error!(
                    "Provided RAM size {} does not match what was expected {}",
                    r.len(),
                    ram_size
                );
                ram = Some(vec![0xFF; ram_size as usize]);
            }
        }

        Mbc5 {
            rom,
            ram,
            rom_bank,
            ram_bank,
            ram_enabled,
            savable,
            has_rumble,
            rumble_active,
        }
    }
}

impl Mbc for Mbc5 {
    fn name(&self) -> String {
        "MBC5".into()
    }

    fn rom(&self) -> &Vec<u8> {
        &self.rom
    }

    fn ram(&self) -> Option<&Vec<u8>> {
        self.ram.as_ref()
    }
}

impl Memory for Mbc5 {
    fn read(&mut self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => self.rom[address as usize],
            0x4000..=0x7FFF => {
                self.rom[0x4000 * (self.rom_bank as usize % self.rom_banks() as usize)
                    + (address as usize - 0x4000)]
            }
            0xA000..=0xBFFF if !self.ram_enabled => 0xFF,
            0xA000..=0xBFFF => {
                if let Some(ram) = self.ram.as_ref() {
                    let effective_address =
                        0x2000 * self.ram_bank as usize + (address as usize - 0xA000);
                    ram[effective_address]
                } else {
                    log::error!(
                        "Read from RAM address {:#6X} for {} MBC with no RAM",
                        address,
                        self.name()
                    );
                    0xFF
                }
            }
            _ => {
                log::error!("Read from {:#6X} for {} MBC", address, self.name());
                0xFF
            }
        }
    }

    fn write(&mut self, address: u16, data: u8) {
        match address {
            // Unlike MBC1 all bits of the written value matter for MBC5
            0x0000..=0x1FFF => self.ram_enabled = data == 0x0A,
            0x2000..=0x2FFF => self.rom_bank = (self.rom_bank & 0x100) | data as u16,
            0x3000..=0x3FFF => self.rom_bank = ((data as u16 & 0b1) << 8) | (self.rom_bank & 0xFF),
            0x4000..=0x5FFF => {
                // The lower 4 bits of the written value are the RAM bank number
                self.ram_bank = data & 0b1111;
                // The bit 3 enables and disables rumble
                // TODO: Take in a RUMBLE callback from the UI code
                self.rumble_active = data & 0b1000 != 0;
            }
            0xA000..=0xBFFF if self.ram_enabled => {
                let effective_address =
                    0x2000 * self.ram_bank as usize + (address as usize - 0xA000);
                if let Some(ram) = self.ram.as_mut() {
                    ram[effective_address] = data;
                }
            }
            0xA000..=0xBFFF => {}
            _ => log::error!(
                "Write to {:#6X} with {:#4X} for {} MBC",
                address,
                data,
                self.name()
            ),
        }
    }
}

impl Savable for Mbc5 {
    fn savable(&self) -> bool {
        self.savable
    }

    fn save_ram(&self) -> Option<&Vec<u8>> {
        self.ram.as_ref()
    }
}

impl Cartridge for Mbc5 {}

// END-MBC5 ----------------------------------------------------------------------------------------

// Helper methods
/// Calculate the ROM size and number of ROM banks of the cartridge from the
/// byte at 0x148. Return this information as a (size, banks) tuple
fn rom_size(value: u8) -> (u32, u32) {
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
fn ram_size(value: u8) -> (u32, u32) {
    match value {
        0x00 => (0x00, 0x00),             // No RAM
        0x02 => (RAM_BANK_SIZE, 1),       // 8KB
        0x03 => (RAM_BANK_SIZE * 4, 4),   // 32KB
        0x04 => (RAM_BANK_SIZE * 16, 16), // 128 KB
        0x05 => (RAM_BANK_SIZE * 8, 8),   // 64KB
        _ => panic!("Unknown RAM size byte {:#4X}", value),
    }
}
