use std::ops::Deref;

use thiserror::Error;

use crate::{memory::Memory, min_number_of_bits, HardwareSupport};

const CGB_FLAG_ADDRESS: u16 = 0x143;
const CARTRIDGE_TYPE_ADDRESS: u16 = 0x147;
const ROM_SIZE_ADDRESS: u16 = 0x148;
const ROM_BANK_SIZE: usize = 1024 * 16;
const RAM_SIZE_ADDRESS: u16 = 0x149;
const RAM_BANK_SIZE: usize = 1024 * 8;

pub const BOOT_ROM_START: u16 = 0x0000;
pub const BOOT_ROM_END: u16 = 0x08FF;
pub const CGB_BOOT_ROM: &[u8; 0x900] = include_bytes!("../roms/cgb_boot.bin");

pub const CART_ROM_START: u16 = 0x0000;
pub const CART_ROM_END: u16 = 0x7FFF;

pub const CART_RAM_START: u16 = 0xA000;
pub const CART_RAM_END: u16 = 0xBFFF;

pub trait Mbc: Memory {
    /// Name of the MBC as determined by the cartridge type
    fn name(&self) -> String;

    fn rom(&self) -> &Vec<u8>;

    fn ram(&self) -> Option<&Vec<u8>> {
        None
    }

    fn savable(&self) -> bool;
    fn save_ram(&self) -> Option<&Vec<u8>>;
}

pub struct CartridgeHeader {
    pub title: String,
    pub manufacturer_code: String,
    pub hardware_supported: HardwareSupport,
    pub cart_type: String,
    rom_size_and_banks: (usize, usize),
    ram_size_and_banks: (usize, usize),
}

impl CartridgeHeader {
    /// ROM size in bytes
    pub fn rom_size(&self) -> usize {
        self.rom_size_and_banks.0
    }

    /// Number of ROM banks
    pub fn rom_banks(&self) -> usize {
        self.rom_size_and_banks.1
    }

    /// RAM size in bytes. 0 if None
    pub fn ram_size(&self) -> usize {
        self.ram_size_and_banks.0
    }

    /// Number of RAM banks
    pub fn ram_banks(&self) -> usize {
        self.ram_size_and_banks.1
    }
}

#[derive(Error, Clone, Debug)]
pub enum CartridgeError {
    #[error("Invalid header. Error: '{0}'")]
    Header(String),
    #[error("Invalid size (expected '{expected}', got '{got}'")]
    Size { expected: usize, got: usize },
}

pub struct Cartridge {
    mbc: Box<dyn Mbc>,
    pub header: CartridgeHeader,
}

impl Cartridge {
    pub fn new(rom: Vec<u8>, ram: Option<Vec<u8>>) -> Result<Self, String> {
        let header = Cartridge::parse_header(&rom[0x100..0x150]).map_err(|e| e.to_string())?;
        let mbc: Box<dyn Mbc> = match rom[CARTRIDGE_TYPE_ADDRESS as usize] {
            0x00 => Box::new(NoMbc::new(rom)),
            code @ (0x01..=0x03) => Box::new(Mbc1::new(rom, ram, code == 0x03, &header)),
            code @ (0x19..=0x1E) => Box::new(Mbc5::new(
                rom,
                ram,
                code == 0x1B || code == 0x1E,
                code == 0x1C || code == 0x1D || code == 0x1E,
                &header,
            )),
            code => return Err(format!("Unsupported MBC with code: '{code}'")),
        };

        Ok(Self { mbc, header })
    }

    fn parse_header(header: &[u8]) -> Result<CartridgeHeader, CartridgeError> {
        if header.len() != 0x50 {
            return Err(CartridgeError::Size {
                expected: 0x50,
                got: header.len(),
            });
        }

        let title = match std::str::from_utf8(&header[0x034..0x03F]) {
            Ok(value) => value.to_string(),
            Err(e) => {
                return Err(CartridgeError::Header(format!(
                    "Error in decoding title: {}",
                    e.to_string()
                )))
            }
        };

        let manufacturer_code = match std::str::from_utf8(&header[0x03F..0x043]) {
            Ok(value) => value.to_string(),
            Err(e) => return Err(CartridgeError::Header(e.to_string())),
        };

        let hardware_supported =
            Cartridge::hardware_supported(header[CGB_FLAG_ADDRESS as usize - 0x100]);
        let cart_type = match header[CARTRIDGE_TYPE_ADDRESS as usize - 0x100] {
            0x00 => "no_mbc".to_string(),
            0x01 => "mbc1".to_string(),
            0x02 => "mbc1 | ram".to_string(),
            0x03 => "mbc1 | ram | battery".to_string(),
            0x05 => "mbc2".to_string(),
            0x06 => "mbc2 | battery".to_string(),
            0x08 => "no_mbc | ram".to_string(),
            0x09 => "no_mbc | ram | battery".to_string(),
            0x0B => "mmm01".to_string(),
            0x0C => "mmm01 | ram".to_string(),
            0x0D => "mmm01 | ram | battery".to_string(),
            0x0F => "mbc3 | timer | battery".to_string(),
            0x10 => "mbc3 | timer | ram | battery".to_string(),
            0x11 => "mbc3".to_string(),
            0x12 => "mbc3 | ram".to_string(),
            0x13 => "mbc3 | ram | battery".to_string(),
            0x19 => "mbc5".to_string(),
            0x1A => "mbc5 | ram".to_string(),
            0x1B => "mbc5 | ram | battery".to_string(),
            0x1C => "mbc5 | rumble".to_string(),
            0x1D => "mbc5 | rumble | ram".to_string(),
            0x1E => "mbc5 | rumble | ram | battery".to_string(),
            0x20 => "mbc6".to_string(),
            0x22 => "mbc7 | sensor | rumble".to_string(),
            0xFC => "camera".to_string(),
            0xFD => "tama5".to_string(),
            0xFE => "huc3".to_string(),
            0xFF => "huc1 | ram | battery".to_string(),
            _ => {
                return Err(CartridgeError::Header(format!(
                    "Unexpected byte for cartridge type: '{}'",
                    header[CARTRIDGE_TYPE_ADDRESS as usize - 0x100]
                )))
            }
        };
        let rom_size_and_banks =
            Cartridge::rom_size_from_header(header[ROM_SIZE_ADDRESS as usize - 0x100]);
        let ram_size_and_banks =
            Cartridge::ram_size_from_header(header[RAM_SIZE_ADDRESS as usize - 0x100]);

        Ok(CartridgeHeader {
            title,
            manufacturer_code,
            hardware_supported,
            cart_type,
            rom_size_and_banks,
            ram_size_and_banks,
        })
    }

    // Helper methods
    /// Calculate the ROM size and number of ROM banks of the cartridge from the
    /// byte at 0x148. Return this information as a (size, banks) tuple
    fn rom_size_from_header(value: u8) -> (usize, usize) {
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
    fn ram_size_from_header(value: u8) -> (usize, usize) {
        match value {
            0x00 => (0x00, 0x00),             // No RAM
            0x02 => (RAM_BANK_SIZE, 1),       // 8KB
            0x03 => (RAM_BANK_SIZE * 4, 4),   // 32KB
            0x04 => (RAM_BANK_SIZE * 16, 16), // 128 KB
            0x05 => (RAM_BANK_SIZE * 8, 8),   // 64KB
            _ => panic!("Unknown RAM size byte {:#4X}", value),
        }
    }

    /// Get the hardware supported by the ROM based of the `CGB_FLAG_ADDRESS`
    fn hardware_supported(value: u8) -> HardwareSupport {
        match value {
            0x80 => HardwareSupport::DmgCgb,
            0xC0 => HardwareSupport::CgbOnly,
            _ => HardwareSupport::DmgCompat,
        }
    }
}

impl Memory for Cartridge {
    fn read(&mut self, address: u16) -> u8 {
        self.mbc.read(address)
    }

    fn write(&mut self, address: u16, data: u8) {
        self.mbc.write(address, data)
    }
}

impl Deref for Cartridge {
    type Target = Box<dyn Mbc>;

    fn deref(&self) -> &Self::Target {
        &self.mbc
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

    fn savable(&self) -> bool {
        false
    }

    fn save_ram(&self) -> Option<&Vec<u8>> {
        None
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
// End-ROM Only MBC---------------------------------------------------------------------------------

// MBC1 --------------------------------------------------------------------------------------------

struct Mbc1 {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,

    rom_bank: u8,
    rom_bit_mask: u8,
    total_rom_banks: usize,

    ram_bank: u8,
    total_ram_banks: usize,
    ram_size: usize,
    ram_enabled: bool,
    ram_banking_mode: bool,

    savable: bool,
}

impl Mbc1 {
    pub fn new(
        rom: Vec<u8>,
        mut ram: Option<Vec<u8>>,
        savable: bool,
        header: &CartridgeHeader,
    ) -> Self {
        let rom_bank = 0x01;
        let ram_bank = 0x00;
        let ram_enabled = false;
        let ram_banking_mode = false;

        let rom_banks = header.rom_banks();
        let rom_bits_required = min_number_of_bits(rom_banks as u8) - 1;
        let rom_bit_mask = (i8::MIN >> (rom_bits_required - 1)) as u8 >> (8 - rom_bits_required);

        let ram_size = header.ram_size();
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
            total_rom_banks: header.rom_banks(),
            ram_bank,
            ram_enabled,
            ram_banking_mode,
            savable,
            total_ram_banks: header.ram_banks(),
            ram_size: header.ram_size(),
        }
    }

    fn effective_ram_address(&self, address: u16) -> usize {
        if self.total_ram_banks > 1 {
            if self.ram_banking_mode {
                0x2000 * self.ram_bank as usize + (address as usize - 0xA000)
            } else {
                // RAM banking not enabled. Use the 0 bank of RAM
                address as usize - 0xA000
            }
        } else {
            // Only one bank of RAM exists either the full 8KB or 2KB (which requires
            // the % RAM_SIZE)
            (address as usize - 0xA000) % self.ram_size
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

    fn savable(&self) -> bool {
        self.savable
    }

    fn save_ram(&self) -> Option<&Vec<u8>> {
        self.ram.as_ref()
    }
}

impl Memory for Mbc1 {
    fn read(&mut self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF if !self.ram_banking_mode => self.rom[address as usize],
            0x0000..=0x3FFF if self.ram_banking_mode => {
                let zero_bank_number = if self.total_rom_banks <= 32 {
                    0x00
                } else if self.total_rom_banks == 64 {
                    (self.ram_bank & 0b1) << 4
                } else {
                    (self.ram_bank << 5) | self.rom_bank
                };

                self.rom[0x4000 * zero_bank_number as usize + address as usize]
            }
            0x4000..=0x7FFF => {
                let high_bank_number = if self.total_rom_banks <= 32 {
                    self.rom_bank
                } else if self.total_rom_banks == 64 {
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
// END-MBC1 ----------------------------------------------------------------------------------------

// MBC5 --------------------------------------------------------------------------------------------

struct Mbc5 {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,

    rom_bank: u16,
    total_rom_banks: usize,

    ram_bank: u8,
    ram_enabled: bool,

    savable: bool,

    has_rumble: bool,
    rumble_active: bool,
}

impl Mbc5 {
    pub fn new(
        rom: Vec<u8>,
        mut ram: Option<Vec<u8>>,
        savable: bool,
        has_rumble: bool,
        header: &CartridgeHeader,
    ) -> Self {
        let rom_bank = 0x01;
        let ram_bank = 0x00;
        let ram_enabled = false;
        let rumble_active = false;

        let ram_size = header.ram_size();
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
            total_rom_banks: header.rom_banks(),
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

    fn savable(&self) -> bool {
        self.savable
    }

    fn save_ram(&self) -> Option<&Vec<u8>> {
        self.ram.as_ref()
    }
}

impl Memory for Mbc5 {
    fn read(&mut self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => self.rom[address as usize],
            0x4000..=0x7FFF => {
                self.rom[0x4000 * (self.rom_bank as usize % self.total_rom_banks)
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
// END-MBC5 ----------------------------------------------------------------------------------------
