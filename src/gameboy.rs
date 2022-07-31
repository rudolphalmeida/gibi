use std::{cell::RefCell, rc::Rc};

use crate::apu::Apu;
use crate::interrupts::InterruptHandler;
use crate::utils::Byte;
use crate::{cpu::Cpu, mmu::Mmu, ppu::Ppu};

pub struct Gameboy {
    mmu: Rc<RefCell<Mmu>>,
    cpu: Cpu,
}

impl Gameboy {
    pub fn new(rom: Vec<Byte>) -> Self {
        let interrupts = Rc::new(RefCell::new(InterruptHandler::default()));

        let ppu = Rc::new(RefCell::new(Ppu::new(Rc::clone(&interrupts))));
        let apu = Rc::new(RefCell::new(Apu::new()));
        let mmu = Rc::new(RefCell::new(Mmu::new(
            rom,
            ppu,
            apu,
            Rc::clone(&interrupts),
        )));
        let cpu = Cpu::new(Rc::clone(&mmu), interrupts);

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
