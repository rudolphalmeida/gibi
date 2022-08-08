use std::{cell::RefCell, rc::Rc};

use crate::apu::Apu;
use crate::interrupts::InterruptHandler;
use crate::utils::{Byte, Cycles};
use crate::{cpu::Cpu, mmu::Mmu, ppu::Ppu};

const CYCLES_PER_FRAME: Cycles = 17556;

pub struct Gameboy {
    mmu: Rc<RefCell<Mmu>>,
    ppu: Rc<RefCell<Ppu>>,
    cpu: Cpu,

    /// Since we run the CPU one opcode at a time or more, each frame can overrun
    /// the `CYCLES_PER_FRAME` (`17556`) value by a tiny amount. However, eventually
    /// these add up and one frame of CPU execution can miss the PPU frame by a
    /// few scanlines. We use this value to keep track of excess cycles in the
    /// previous frame and ignore those many in the current frame
    carry_over_cycles: Cycles,
}

impl Gameboy {
    pub fn new(rom: Vec<Byte>, ram: Option<Vec<Byte>>) -> Self {
        let interrupts = Rc::new(RefCell::new(InterruptHandler::default()));

        let ppu = Rc::new(RefCell::new(Ppu::new(Rc::clone(&interrupts))));
        let apu = Rc::new(RefCell::new(Apu::new()));
        let mmu = Rc::new(RefCell::new(Mmu::new(
            rom,
            ram,
            Rc::clone(&ppu),
            apu,
            Rc::clone(&interrupts),
        )));
        let cpu = Cpu::new(Rc::clone(&mmu), interrupts);
        let carry_over_cycles = 0;

        log::debug!("Initialized GameBoy with DMG components");
        {
            let mmu = mmu.borrow();
            log::info!("Loaded a cartridge with MBC: {}", mmu.cart.name());
            log::info!("Number of ROM banks: {}", mmu.cart.rom_banks());
            log::info!("ROM size (Bytes): {}", mmu.cart.rom_size());
            log::info!("Number of RAM banks: {}", mmu.cart.ram_banks());
            log::info!("RAM size (Bytes): {}", mmu.cart.ram_banks());
        }
        Gameboy {
            mmu,
            cpu,
            ppu,
            carry_over_cycles,
        }
    }

    pub fn run_one_frame(&mut self) {
        let machine_cycles = self.mmu.borrow().cpu_m_cycles.get();
        let target_machine_cycles = machine_cycles + CYCLES_PER_FRAME - self.carry_over_cycles;

        while self.mmu.borrow().cpu_m_cycles.get() < target_machine_cycles {
            self.cpu.execute();
        }

        self.carry_over_cycles = self.mmu.borrow().cpu_m_cycles.get() - target_machine_cycles;
    }

    pub fn copy_framebuffer_to_draw_target(&self, buffer: &mut [Byte]) {
        self.ppu.borrow().copy_framebuffer_to_draw_target(buffer);
    }
}
