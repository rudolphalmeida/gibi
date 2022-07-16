use crate::{mmu::Mmu, options::Options};

pub struct Gameboy {
    mmu: Mmu,
    // cpu: CPU
}

impl Gameboy {
    pub fn new(options: &Options) -> Self {
        Gameboy {
            mmu: Mmu::new(options),
        }
    }

    pub fn run(&mut self) {
        log::info!("Loaded a cartridge with MBC: {}", self.mmu.cart.name());
    }
}
