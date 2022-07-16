use crate::{
    cartridge::{load_from_file, Cartridge},
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
        let wram = Vec::with_capacity(WRAM_BANK_SIZE * 2); // 8KB
        let hram = Vec::with_capacity(HRAM_SIZE);

        log::debug!("Initialized MMU for DMG");

        Self { cart, wram, hram }
    }

    fn tick(&mut self) {
        // Do nothing for now
    }

    // Read and Write for Mmu. The `Memory` trait is not implemented for the Mmu
    // because `read` here needs to take a mutable reference to `self` due to
    // using `tick` inside it. We want the other components to keep up with the
    // CPU during each memory access
    fn read(&mut self, address: Word) -> Byte {
        self.tick();
        match address {
            0x0000..=0x7FFF => return self.cart.read(address),
            0x8000..=0x9FFF => log::info!("Read from PPU VRAM {:#6X}", address),
            0xA000..=0xBFFF => return self.cart.read(address),
            0xC000..=0xDFFF => return self.wram[address as usize - 0xC000],
            0xE000..=0xFDFF => return self.wram[address as usize - 0xE000],
            0xFE00..=0xFE9F => log::info!("Read from PPU OAM {:#6X}", address),
            0xFEA0..=0xFEFF => log::error!("Read from unused area {:#6X}", address),
            0xFF00..=0xFF7F => log::info!("Read from IO register {:#6X}", address),
            0xFF80..=0xFFFE => return self.hram[address as usize - 0xFF80],
            0xFFFF => log::info!("Read from IE register {:#6X}", address),
        }

        0xFF
    }

    fn write(&mut self, address: Word, data: Byte) {
        self.tick();
        match address {
            0x0000..=0x7FFF => self.cart.write(address, data),
            0x8000..=0x9FFF => log::info!("Write to PPU VRAM {:#6X} with {:#4X}", address, data),
            0xA000..=0xBFFF => self.cart.write(address, data),
            0xC000..=0xDFFF => self.wram[address as usize - 0xC000] = data,
            0xE000..=0xFDFF => self.wram[address as usize - 0xE000] = data,
            0xFE00..=0xFE9F => log::info!("Read from PPU OAM {:#6X}", address),
            0xFEA0..=0xFEFF => {
                log::error!("Write to unused area {:#6X} with {:#4X}", address, data)
            }
            0xFF00..=0xFF7F => log::info!("Write to IO register {:#6X} with {:#4X}", address, data),
            0xFF80..=0xFFFE => self.hram[address as usize - 0xFF80] = data,
            0xFFFF => log::info!("Write to IE register {:#6X} with {:#4X}", address, data),
        }
    }
}
