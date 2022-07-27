use std::{cell::RefCell, rc::Rc};

use crate::apu::Apu;
use crate::{cpu::Cpu, mmu::Mmu, options::Options, ppu::Ppu};

pub struct Gameboy {
    mmu: Rc<RefCell<Mmu>>,
    cpu: Cpu,
}

impl Gameboy {
    pub fn new(options: &Options) -> Self {
        let ppu = Rc::new(RefCell::new(Ppu::new()));
        let apu = Rc::new(RefCell::new(Apu::new()));
        let mmu = Rc::new(RefCell::new(Mmu::new(options, ppu, apu)));
        let cpu = Cpu::new(Rc::clone(&mmu));

        log::debug!("Initialized GameBoy with DMG components");
        Gameboy { mmu, cpu }
    }

    pub fn run(&mut self) {
        log::info!(
            "Loaded a cartridge with MBC: {}",
            self.mmu.borrow().cart.name()
        );

        loop {
            self.cpu.execute();
        }
    }
}
