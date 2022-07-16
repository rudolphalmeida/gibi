use std::{cell::RefCell, rc::Rc};

use crate::{cpu::Cpu, mmu::Mmu, options::Options};

pub struct Gameboy {
    mmu: Rc<RefCell<Mmu>>,
    cpu: Cpu,
}

impl Gameboy {
    pub fn new(options: &Options) -> Self {
        let mmu = Rc::new(RefCell::new(Mmu::new(options)));
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
            self.cpu.execute_opcode();
        }
    }
}
