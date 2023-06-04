use std::path::PathBuf;
use std::{cell::RefCell, rc::Rc};
use std::{fs, io};

use crate::apu::Apu;
use crate::cartridge::init_mbc_from_rom;
use crate::interrupts::InterruptHandler;
use crate::joypad::{Joypad, JoypadKeys};
use crate::{cpu::Cpu, mmu::Mmu, ppu::Ppu};
use crate::{ExecutionState, HardwareSupport, SystemState};

const CYCLES_PER_FRAME: u64 = 17556;

pub struct Gameboy {
    mmu: Rc<RefCell<Mmu>>,
    ppu: Rc<RefCell<Ppu>>,
    joypad: Rc<RefCell<Joypad>>,
    cpu: Cpu,

    system_state: Rc<RefCell<SystemState>>,
}

impl Gameboy {
    pub fn new(rom: Vec<u8>, ram: Option<Vec<u8>>) -> Self {
        let cart = init_mbc_from_rom(rom, ram);
        let hardware_support = cart.hardware_supported();

        match hardware_support {
            HardwareSupport::CgbOnly => log::info!("Game supports CGB hardware only"),
            HardwareSupport::DmgCgb => log::info!("Game supports both CGB and DMG"),
            HardwareSupport::DmgCompat => log::info!("Game is running in DMG compatability mode"),
        }

        let system_state = Rc::new(RefCell::new(SystemState {
            execution_state: ExecutionState::ExecutingBootrom,
            hardware_support,
            carry_over_cycles: 0,
            total_cycles: 0,
        }));

        let interrupts = Rc::new(RefCell::new(InterruptHandler::default()));

        let ppu = Rc::new(RefCell::new(Ppu::new(
            Rc::clone(&interrupts),
            Rc::clone(&system_state),
        )));
        let apu = Rc::new(RefCell::new(Apu::new()));
        let joypad = Rc::new(RefCell::new(Joypad::new(Rc::clone(&interrupts))));
        let mmu = Rc::new(RefCell::new(Mmu::new(
            cart,
            Rc::clone(&system_state),
            Rc::clone(&ppu),
            apu,
            Rc::clone(&joypad),
            Rc::clone(&interrupts),
        )));
        let cpu = Cpu::new(Rc::clone(&mmu), interrupts, Rc::clone(&system_state));

        {
            let mmu = mmu.borrow();
            log::info!("Loaded a cartridge with MBC: {}", mmu.cart.name());
            log::info!("Number of ROM banks: {}", mmu.cart.rom_banks());
            log::info!("ROM size (Bytes): {}", mmu.cart.rom_size());
            log::info!("Number of RAM banks: {}", mmu.cart.ram_banks());
            log::info!("RAM size (Bytes): {}", mmu.cart.ram_size());
        }
        Gameboy {
            system_state,
            mmu,
            cpu,
            joypad,
            ppu,
        }
    }

    pub fn run_one_frame(&mut self) {
        let machine_cycles = self.system_state.borrow().total_cycles;
        let target_machine_cycles = machine_cycles
            + CYCLES_PER_FRAME * self.mmu.borrow().speed_multiplier()
            - self.system_state.borrow().carry_over_cycles;

        while self.system_state.borrow().total_cycles < target_machine_cycles {
            self.cpu.execute();
        }

        let carry_over_cycles = self.system_state.borrow().total_cycles - target_machine_cycles;
        self.system_state.borrow_mut().carry_over_cycles = carry_over_cycles;
    }

    pub fn copy_framebuffer_to_draw_target(&self, buffer: &mut [u8]) {
        self.ppu.borrow().copy_framebuffer_to_draw_target(buffer);
    }

    pub fn keydown(&mut self, key: JoypadKeys) {
        self.joypad.borrow_mut().keydown(key);
    }

    pub fn keyup(&mut self, key: JoypadKeys) {
        self.joypad.borrow_mut().keyup(key);
    }

    pub fn save(&self, path: PathBuf) -> io::Result<String> {
        if let Some(ram) = self.mmu.borrow().save_ram() {
            fs::write(path, ram).map(|_| "Save RAM to file".into())
        } else {
            Ok("Game does not have battery-backed saves".into())
        }
    }
}
