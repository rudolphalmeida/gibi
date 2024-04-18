use std::path::PathBuf;

use std::{cell::RefCell, rc::Rc};
use std::{fs, io};

use crate::cartridge::init_mbc_from_rom;
use crate::cpu::Registers;
use crate::framebuffer::access;
use crate::interrupts::InterruptHandler;
use crate::joypad::{Joypad, JoypadKeys};
use crate::{cpu::Cpu, mmu::Mmu, GameFrame};
use crate::{ExecutionState, HardwareSupport, HdmaState, SystemState};

const CYCLES_PER_FRAME: u64 = 17556;

pub struct Gameboy {
    mmu: Mmu,
    joypad: Rc<RefCell<Joypad>>,
    cpu: Cpu,

    system_state: Rc<RefCell<SystemState>>,
}

impl Gameboy {
    pub fn new(rom: Vec<u8>, ram: Option<Vec<u8>>) -> Self {
        let cart = init_mbc_from_rom(rom, ram);
        let hardware_support = cart.hardware_supported();

        log::info!("Loaded a cartridge with MBC: {}", cart.name());
        log::info!("Number of ROM banks: {}", cart.rom_banks());
        log::info!("ROM size (Bytes): {}", cart.rom_size());
        log::info!("Number of RAM banks: {}", cart.ram_banks());
        log::info!("RAM size (Bytes): {}", cart.ram_size());

        match hardware_support {
            HardwareSupport::CgbOnly => log::info!("Game supports CGB hardware only"),
            HardwareSupport::DmgCgb => log::info!("Game supports both CGB and DMG"),
            HardwareSupport::DmgCompat => log::info!("Game is running in DMG compatibility mode"),
        }

        let system_state = Rc::new(RefCell::new(SystemState {
            execution_state: ExecutionState::ExecutingProgram,
            hardware_support,
            carry_over_cycles: 0,
            total_cycles: 0,
            key1: 0x00,
            bootrom_mapped: true,
            hdma_state: HdmaState {
                source_addr: 0xFFFF,
                dest_addr: 0xFFFF,
                hdma_stat: 0x00,
            },
        }));

        let interrupts = Rc::new(RefCell::new(InterruptHandler::default()));
        let joypad = Rc::new(RefCell::new(Joypad::new(Rc::clone(&interrupts))));
        let mmu = Mmu::new(
            cart,
            Rc::clone(&system_state),
            Rc::clone(&joypad),
            Rc::clone(&interrupts),
        );
        let cpu = Cpu::new(interrupts, Rc::clone(&system_state));
        Gameboy {
            system_state,
            mmu,
            cpu,
            joypad,
        }
    }

    // TODO: Extract out a debug info type
    pub fn send_debug_data(&self) -> Registers {
        self.cpu.regs
    }

    pub fn run_one_frame(&mut self) {
        let machine_cycles = self.system_state.borrow().total_cycles;
        let carry_over_cycles = self.system_state.borrow().carry_over_cycles;
        let speed_multiplier = self.system_state.borrow().speed_multiplier();

        let target_machine_cycles =
            machine_cycles + CYCLES_PER_FRAME * speed_multiplier - carry_over_cycles;

        while self.system_state.borrow().total_cycles < target_machine_cycles {
            self.cpu.execute(&mut self.mmu);
        }

        let carry_over_cycles = self.system_state.borrow().total_cycles - target_machine_cycles;
        self.system_state.borrow_mut().carry_over_cycles = carry_over_cycles;
    }

    pub fn write_frame(&self, frame_writer: &mut access::AccessW<GameFrame>) {
        self.mmu.ppu.borrow().write_frame(frame_writer);
    }

    pub fn keydown(&mut self, key: JoypadKeys) {
        self.joypad.borrow_mut().keydown(key);
    }

    pub fn keyup(&mut self, key: JoypadKeys) {
        self.joypad.borrow_mut().keyup(key);
    }

    pub fn save(&self, path: &PathBuf) -> io::Result<String> {
        if let Some(ram) = self.mmu.save_ram() {
            fs::write(path, ram).map(|_| "Save RAM to file".into())
        } else {
            Ok("Game does not have battery-backed saves".into())
        }
    }
}
