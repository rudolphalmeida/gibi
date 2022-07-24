use crate::{
    cartridge::{load_from_file, Cartridge, BOOT_ROM},
    memory::Memory,
    options::Options,
    utils::{Byte, Word},
};

const WRAM_BANK_SIZE: usize = 1024 * 4; // 4KB
const HRAM_SIZE: usize = 0xFFFE - 0xFF80 + 1;

/// The MMU (Memory-Management Unit) is responsible for connecting the CPU to
/// the rest of the components. The `Mmu` implements the memory-map of the
/// GameBoy, redirecting reads and writes made by the CPU and PPU to the
/// correct component to which the address is mapped.
pub(crate) struct Mmu {
    pub(crate) cart: Box<dyn Cartridge>,
    wram: Vec<Byte>,
    hram: Vec<Byte>,
}

impl Mmu {
    pub fn new(options: &Options) -> Self {
        let cart = load_from_file(options).unwrap();
        let wram = Vec::from([0x00; WRAM_BANK_SIZE * 2]); // 8KB
        let hram = Vec::from([0x00; HRAM_SIZE]);

        log::debug!("Initialized MMU for DMG");

        Self { cart, wram, hram }
    }

    pub fn tick(&self) {
        // Do nothing for now
    }

    /// Raw Read: Read the contents of a memory location without ticking all the
    /// components
    pub fn raw_read(&self, address: u16) -> Byte {
        match address {
            0x0000..=0x0100 => return BOOT_ROM[address as usize],
            0x0101..=0x7FFF => return self.cart.read(address),
            0x8000..=0x9FFF => log::info!("Read from PPU VRAM {:#06X}", address),
            0xA000..=0xBFFF => return self.cart.read(address),
            0xC000..=0xDFFF => return self.wram[address as usize - 0xC000],
            0xE000..=0xFDFF => return self.wram[address as usize - 0xE000],
            0xFE00..=0xFE9F => log::info!("Read from PPU OAM {:#06X}", address),
            0xFEA0..=0xFEFF => log::error!("Read from unused area {:#06X}", address),
            0xFF00..=0xFF7F => log::info!("Read from IO register {:#06X}", address),
            0xFF80..=0xFFFE => return self.hram[address as usize - 0xFF80],
            0xFFFF => log::info!("Read from IE register {:#06X}", address),
        }
        0xFF
    }

    fn raw_write(&mut self, address: Word, data: Byte) {
        match address {
            0x0000..=0x0100 => log::error!("Write to boot ROM {:#06X} with {:#04X}", address, data),
            0x0101..=0x7FFF => self.cart.write(address, data),
            0x8000..=0x9FFF => log::info!("Write to PPU VRAM {:#06X} with {:#04X}", address, data),
            0xA000..=0xBFFF => self.cart.write(address, data),
            0xC000..=0xDFFF => self.wram[address as usize - 0xC000] = data,
            0xE000..=0xFDFF => self.wram[address as usize - 0xE000] = data,
            0xFE00..=0xFE9F => log::info!("Read from PPU OAM {:#06X}", address),
            0xFEA0..=0xFEFF => {
                log::error!("Write to unused area {:#6X} with {:#04X}", address, data)
            }
            0xFF00..=0xFF7F => {
                log::info!("Write to IO register {:#06X} with {:#04X}", address, data)
            }
            0xFF80..=0xFFFE => self.hram[address as usize - 0xFF80] = data,
            0xFFFF => log::info!("Write to IE register {:#06X} with {:#04X}", address, data),
        }
    }
}

impl Memory for Mmu {
    /// Read and Write for Mmu. The `Memory` trait is not implemented for the Mmu
    /// because `read` here needs to take a mutable reference to `self` due to
    /// using `tick` inside it. We want the other components to keep up with the
    /// CPU during each memory access
    fn read(&self, address: Word) -> Byte {
        self.tick();
        self.raw_read(address)
    }

    fn write(&mut self, address: Word, data: Byte) {
        self.tick();
        self.raw_write(address, data);
    }
}
